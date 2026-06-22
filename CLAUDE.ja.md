<!-- これは CLAUDE.md（英語）の日本語版です。ツールが読み込むのは CLAUDE.md の方です。 -->

# Visitor システム — 現状

AST 型定義から `syn` スタイルのビジターを生成する。

コード: `core/src/visit.rs`（`Ast`, `Repeater` トレイト）、`macro/ast.rs`（`#[derive(Ast)]`）、
`macro/visitor.rs`（`visitor!` シム → `__visitor_entry` → `__visitor_build`）。テスト:
`core/tests/visitor_*.rs`、`core/tests/ast_derive.rs`、`core/tests/ast_recurse.rs`；クレート跨ぎ:
`rust/tests/cross_crate.rs`（AST サンプルは `rust/src/lib.rs`）。

## 実装済み・テスト済み

- **`#[derive(Ast)]`**: 空の `Ast` マーカー impl ＋ `#[macro_export]` なコールバック「メタデータ」
  マクロ（定義のクリーンな複製を保持し、下流で `syn::Item` として再パースされる）。型自身の名前で
  再エクスポートされ `path::to::T!{..}` として到達可能（型とマクロは別の名前空間なので共存できる）。
  さらに、型コンテキストに依存する各フィールド型につき `type_leak::Leaker` マーカー ＋
  `::syan::visit::Repeater<N>` impl を 1 つ生成する（ポータブルな参照；消費者は drill-in / 外部の
  メタデータ利用者）。
- **`visitor!([base =>] T, …)`**: （それ以外は空の）`mod` の**内側**で呼ぶ関数形式マクロ。`syan`
  内の `macro_rules!` シムが `$crate` を捕捉して syan のパスを `__visitor_entry` に渡すので
  `#[syan(..)]` は不要。`__visitor_build` を介したメタデータの往復（ping-pong）で、visit 対象の型ごとに
  `Visit`/`VisitMut` のトレイトメソッド、フリー関数 `visit_*`/`visit_*_mut`、そして型上の
  **インヒーレント** `visit`/`visit_mut` メソッドを生成する（呼び出し側でのトレイト import は不要；ただし
  ビジターは対象型と同じクレートで生成する必要がある）。型引数はビジターモジュールの内側から解決できる
  パスで書く（`super::Expr`、`crate::ast::Expr`）。直接フィールドと `Box` 包みの AST フィールドは辿る；
  それ以外のヘッド（`Vec`/`Option` を含む）は現状リーフ — drill-in 計画を参照。
- **入力**（`IntoVisitor`/`IntoVisitorMut` のセレクタ設計）: 構造体ビジター（`&mut` 経由）、単一
  クロージャ、そして**クロージャのタプル**（アリティ 2..=8）を**1 回**の走査で実行（浅い `Hook` ＋
  単一パスの `Driver` ＋ `Chain`）。
- **`visit_mut`** の完全な鏡像（インプレース変更）。**削除/追加（reduce/append）**: *親*ノードの
  `visit_*_mut` をオーバーライドし（親が `&mut Vec`/`&mut Option` を所有）、その後 descend する —
  `visitor_reduce.rs` 参照。
- **継承** `visitor!(base => New)`: new→base の参照 DAG 向け（base が `__syan_visited` リストマクロを
  エクスポートし、new トレイトがスーパートレイトで拡張する）。
- **ジェネリクス**: トレイトは visit 対象型のジェネリックパラメータの**和集合**でパラメータ化される
  （`Visit<S, Tokens>`）；各型は自分の部分集合を使うので、アリティ混在も動く（`visitor_generics.rs`）。
  注意: 和集合の全パラメータを使わないルート型で `.visit()` を呼ぶと turbofish が要ることがある。
- **クレート跨ぎ**を検証済み: visit 対象型は `visitor!(...)` に渡したフルパスで名指しされるので、下流
  クレートで import 不要（`rust/tests/cross_crate.rs`）。

## 既知のギャップ

- **コンテナ ＋ drill-in** が未実装の機能 — 下記の次期計画。
- **`#[recurse]` エイリアス上のビジター**: `#[recurse]` 内の `#[derive(Ast)]` は*リネーム後*の内部型に
  適用されるため、メタデータマクロはその名前の下にあり、公開エイリアスの下にはない。`Ast` マーカー自体は
  エイリアスでも成立する（`ast_recurse.rs`）；エイリアス上でビジターを構築するには、メタデータマクロが
  エイリアス名で到達可能である必要がある — 将来対応。

---

# Drill-in 実装計画（選択的 `visit_*` ＋ 推移的ドリルスルー）

## 背景

drill-in の目的は、ビジターが `visit_*` を持たない Ast 型を*通り抜けて*走査することだ（仕様の
`Expr::Cast(cast) => this.visit_type(&cast.0)`）。モデルは**2 つのリスト**を使う:

- **`#[subast(...)]`**（`#[derive(Ast)]` の型ごと） — その型の Ast *の子*: どのフィールドを*辿る*か。
  フィールドは、その（コンテナを剥がした）ヘッドがそこに列挙されているとき（または型自身の型 — 自己再帰は
  暗黙）に限り辿る；**それ以外のフィールド型はすべて無視**（リーフ: トークン、プリミティブ、
  `PhantomData`、span 等）。entry はそのサブ AST のメタデータマクロへの解決可能なパスも供給する
  （メンバシップ＋パス解決を 1 属性で）。`#[no_ast]` は無い。
- **`visitor!(T, …)`**（ビジターごと） — *visited 集合*: 辿る型のうちどれが `visit_*` **メソッド**を
  得るか。辿るフィールドのうちヘッドが **`visitor!(...)` に列挙**されているものは
  `this.visit_<head>(field)` に下がる；列挙**されていない**もの（*未列挙の中間型*、例えば `Cast`）は
  **インラインでドリルスルーされる** — そのメタデータマクロを呼び出して内側に入れ子になった列挙済みの型に
  到達し（`this.visit_type(&cast.0)`）、`visit_cast` は**得ない**。

よって `visit_*` は `visitor!(...)` に名指しされた型にのみ定義される；`#[derive(Ast)]` の全ての型は
依然メタデータマクロを出すが、未列挙の型のマクロが呼ばれるのは、その型を `#[subast]` に列挙する型を処理
している間だけ（＝それをドリルスルーするため）。鍵となる事実: 「この辿るヘッドは呼び出すべき Ast 型か？」
にマクロ展開時の存在判定は要らない — `#[subast]` が既に宣言している（そしてパスも与える）。`Repeater`
impl（`#[derive(Ast)]` が出す）はフォールバックの消費者: ポータブルなサブ AST 参照。

## `#[derive(Ast)]`（`macro/ast.rs`）

`#[subast(...)]` が分類とパス解決の唯一の源。ヘルパー属性:
`#[proc_macro_derive(Ast, attributes(syan, subast))]`。

- **`#[subast(<paths>)]`** — この型のサブ AST 型の型レベル**アローリスト**。各 entry は**定義元の
  モジュール**で解決可能なパス（形式: `crate::` / `::abs::` / `super::` / `self::` / 同一モジュールの
  裸の兄弟 / `b::Foo as BFoo`；ジェネリック引数は書かない）。フィールドは、そのコンテナを剥がしたヘッドが
  列挙 entry に（末尾セグメントで、エイリアス考慮で）一致するとき**に限り** Ast の子であり、その entry の
  パスがそのフィールドの解決可能パスになる。**自己再帰は暗黙**: ヘッドが型自身の型であるフィールドは常に
  辿る（パスは下記 `@SELF`）；自分自身は列挙しない。**それ以外のフィールドはすべて無視**（`_` に束縛、
  リーフ、走査しない）。`#[no_ast]` は存在しない（フィールド/型単位のリーフ印は無い）。
- **トレードオフ（指示による）**: アローリストは明示的で自己文書的（「これらが私のサブ AST で、どこに
  あるか」）であり、パス解決を一様に解く。受け入れたコストは*静かな取りこぼし*の失敗モード — サブ AST を
  列挙し忘れると、そこへの走査が静かに止まる（ディナイリストの「cannot find macro」という声の大きい失敗に
  対して）。`unused entry` 警告と、任意で「この `#[derive(Ast)]` 型は何も辿らない」リントが打ち間違いを
  緩和する。
- メタデータマクロは variant → field ごとにレコードを出す: **辿る**フィールドは **アクセサ**（タプル
  添字 / 名前付き ident）、**コンテナ**（`direct`/`box`/`vec`/`vecdeque`/`option`/`slice`/`punctuated`）、
  内側の**ヘッド ident**（リテラル — proc-macro 側で `visit_<snake(head)>` を構築するため。`macro_rules!`
  では*決して*作らない）、そして**解決済みパス**（entry のパス；自己参照ヘッドは `@SELF` とし、
  `__visitor_build` が「その型を取得したパス」で置換）を持つ；**無視**するフィールドは `@leaf` だけを持つ。
- 定義サイトの診断: `#[subast]` の 2 つの entry が同じ末尾セグメント（裸のフィールドヘッドが曖昧）⇒
  **エラー**（ヒント: どちらかをエイリアス、`b::Foo as BFoo`）；どのフィールドにも一致しない entry ⇒
  **警告**。
- `Leaker` ＋ `Repeater<N>` impl は最終手段の型ネーマ / 外部メタデータ消費者向けに維持する；ビジター
  経路は `#[subast]` で解決したパスを使う。

## モデル — 選択的 `visit_*`、それ以外はドリルスルー

`visitor!(T, …)` は **visited 集合**を列挙する；`__visitor_build` は**その型にのみ**
`visit_*`/`visit_*_mut` を生成する。visited 型の本体生成は、その `#[subast]` フィールドを辿る:
- ヘッド ∈ visited 集合 ⇒ `this.visit_<head>(access)` — メソッド呼び出し；再帰はトレイト経由で回り、
  再帰的/循環的な*visited*型は現状の visited 型走査と全く同様に扱える。
- ヘッドが辿る**中間型**（∈ `#[subast]`、∉ visited）⇒ **インラインドリル**: その中間型のメタデータ
  マクロを呼び、アクセサを延長して（`&cast.0`、`&cast.0.1`、…）*その* `#[subast]` フィールドへ同じ規則で
  再帰する — よって未列挙のラッパに任意の深さで入れ子になった列挙済みの型に到達する
  （`Expr::Cast(c) => this.visit_type(&c.0)`）が、ラッパは `visit_cast` を**得ない**。
- 辿らないヘッド（∉ `#[subast]`、自己でもない）⇒ リーフ、`_` に束縛。

**循環ガード:** インラインドリルは展開中の中間型のスタックを保持する；*未列挙*中間型の循環
（`Cast`→`Cast`、または `A`→`B`→`A`、どれも visited でない）はインライン展開できない ⇒ `__visitor_build`
の**エラー**（「`visitor!(...)` にどれか 1 つを列挙」してメソッド呼び出しで再帰を断つ）。*visited* 型を
通る再帰は問題ない — メソッド呼び出しであってインラインではない。visited 型に到達せず*有限*でリーフに
底をつくドリル部分木は**エラーではない** — `visit_*` 呼び出しを生成しないだけ；未列挙中間型の*循環*
（無限展開）のみがエラー。

## `__visitor_build`（`macro/visitor.rs`）

- **列挙された型にのみ生成。** `visit_*`/`visit_*_mut` のフリー関数＋トレイトメソッドは
  `visitor!(...)` に列挙された型にのみ生成する；1 回限りの項目（`Visit`/`VisitMut` トレイト、
  `Driver`/`Hook`/`Chain`、`IntoVisitor`/`IntoVisitorMut`、インヒーレント `visit`/`visit_mut`）も同様で、
  列挙型の **name-list**（ident ＋ path ＋ own-generics）から最後に構築する。
- **本体のローワリング**（出力中の型の `#[subast]` フィールドごと；コンテナを剥がして）:
  - ヘッド ∈ visited 集合 ⇒ `this.visit_<head>(access)` — メソッド名はリテラルのヘッド ident から
    proc-macro 内で構築（`to_snake`；`macro_rules!` では決して作らない）；その型自身の match スクルティニーは
    `@SELF`（取得したパス）を使う。
  - ヘッドが辿る中間型（∈ `#[subast]`、∉ visited）⇒ **インラインドリル**（アクセサを延長して*その*
    `#[subast]` フィールドへ再帰；その match スクルティニーはその `#[subast]` 解決パスを使う；循環ガードは
    *モデル*の通り）。
  - ヘッド ∉ `#[subast]`（`@leaf`）⇒ `_` に束縛してスキップ。
- **探索 / ping-pong。** メンバシップ（visited か？辿るか？）は proc-macro 側で決める — visited 集合を
  保持し、`#[subast]` レコードが follow/leaf ＋ 解決パスを運ぶ。列挙型と、ドリルのため到達する各未列挙
  中間型を、レコードの**解決パス**で取得する（`@SELF` は現在の型 — 決して取得/enqueue しない）。fetch の
  重複排除は**完全な解決パス文字列**で行い、末尾セグメントでは行わない。よって末尾セグメントが同じ別々の型
  （`a::Cast` と `b::Cast`）は両方とも取得される。`path_of` も推論も無し。

## 解決済みの決定（深掘り）

### Decision 1 — コンテナ: **対象に含める。**
`Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` に潜れないビジターは実 AST（ブロック、item リスト、任意の
サブ式）では使い物にならない。以前の削除は*seq/opt メソッド*の仕組みを落としたものであり、コンテナ走査
という目的を否定したわけではない。認識する集合: `Box`、`Vec`、`VecDeque`、`Option`、`[T]` / `Box<[T]>`、
そして syan の `Punctuated`（拡張可能）。ローワリング: `Box` は deref；`Vec`/slice/`Punctuated` は
`for x in &…`（mut 側は `&mut`）；`Option` は `if let Some(x) = …`；その後に内側ヘッドを visit。内側
ヘッドは、その型が `#[subast]` に列挙されているとき**に限り**辿る（未列挙の型のコンテナ、例えば
`Vec<Token>` は無視）。reduce/append は不変: 親の `visit_*_mut`（`&mut Vec` / `&mut Option` を所有）を
オーバーライドする。

### Decision 2 — 委譲とモデル: **proc-macro が組み立てる；選択的ドリル。**
2 つの厳然たる `macro_rules!` の事実が決め手: (1) 2 つの ident の等価**比較ができない**（`$a == $b` は
無く、matcher 内でメタ変数名を再利用するとエラー）→ visit 集合のメンバシップ判定ができない；(2) ident の
**連結/snake_case 化ができない**（`format_ident!` が無い）→ `Stmt` から `visit_stmt` を作れない。よって
メンバシップ*も*本体生成（メソッド名、match アーム）*も* proc-macro 側に置くしかない。「`macro_rules!`
で委譲」は、各型の構造をメタデータマクロが*供給する*こととして実現される（ping-pong の取得そのものが
委譲）；`__visitor_build` が組み立て、`macro_rules!` にできないこと — visited 集合のメンバシップ判定、
`to_snake` でのメソッド名、インラインドリルの再帰＋循環ガード — を行う。**選択的ドリル**を採る（指示
による）: `visit_*` は `visitor!(...)` に列挙された型にのみ、未列挙の `#[subast]` 型はインラインで
ドリルスルー — visit-all より小さく意図的なインターフェイス（`Cast` は visit 不可；visit したいノードだけ
を正確に公開する）。2 つのリストは別々のまま: `#[subast]`（型ごと）が**follow リスト**（＋パス解決）、
`visitor!(...)` が**メソッドリスト**。

### スケール。
`visitor!(...)` に列挙された型だけがメソッドを得るので、トレイト / `Driver` / `IntoVisitor` /
インヒーレント項目はその（小さく明示的な）集合に対してのみで、列挙型の **name-list**（ident ＋ path ＋
own-generics）から最後に一度だけ出力する（Rust は項目順が自由なので、本体が最後に出るトレイトを参照して
よい）。列挙型の `visit_*` 本体は、その**ドリル閉包**（その型から visited 型かリーフに達するまでに到達する
未列挙中間型）が取得され次第出力される；中間型の構造は**インラインドリルに使って捨てる** — メソッドには
せず、トレイトにも育てない。取得した構造を各 ping-pong バウンスで再出力しない（それは O(N²)）；持ち回るのは
name-list ＋ 現在のドリル閉包だけ。トレイトの**和集合ジェネリクス ＝ ルート型のジェネリックパラメータ**
（早期に判明するのでクロージャも動く）；新たなジェネリックパラメータ名を導入するサブ型はエラー。

### パス解決 — `#[subast]` が辿る全てのパスを供給する。
辿られる各フィールドの解決可能パスは、一致する `#[subast]` entry のパス — そのサブ AST のメタデータマクロを
**取得**するため（`visit_*` 呼び出しになる場合もドリルされる場合も）、そしてドリルされるときの**match
スクルティニー**として使う。自己再帰は `@SELF`（現在の型を取得したパス；match スクルティニーのみ、探索の
辺としては決して enqueue しない）を使う。未列挙のフィールド型は無視されるので、それらに推論すべきパスは無い。
`#[subast]` のパスは*定義元*モジュールで解決され、ポータブルに再公開される — 1 つの `pub use` が**型と
メタデータマクロの両名前空間**を運び、`#[macro_export]` 側を `$crate` 根付けすることで同一クレートでも下流
でも解決する。モジュール前置の推論も兄弟の当て推量も無い: entry の欠落/打ち間違いは*静かに無視される*
フィールド（指示が受け入れた失敗モード）であり、`unused entry` 警告 / 「何も辿らない」リントでしか
捕まらず、誤解決のパスにはならない。残る穴: `visitor!(...)` の*エントリ*パス自体が非正準な再エクスポートの
ときにサブ AST の*型*を名指す件 — `visitor!(...)` のエントリパスを正準（`crate::`/`super::` 根付け）に
要求することで塞ぐ（これにより、継続中の TODO どおり `Leaker` を落とせる）。

## テスト（`core/tests`）

- 仕様グラフ: `Type`、`Cast(Type)`、`Expr { Cast(Cast) }`（`Expr` に `#[subast(super::Cast)]`、`Cast` に
  `#[subast(super::Type)]`）；`visitor!(super::Expr, super::Type)`（Type は列挙、**Cast は非列挙**）⇒
  `visit_expr` は `Cast` を通って `this.visit_type(&cast.0)` までドリルする；`Cast` は visit **不可**
  （`visit_cast` 無し）；クロージャ `|t: &Type<()>| …` が 1 回発火する。
- follow リスト対メソッドリスト: 辿られて列挙されたフィールド（`#[subast(crate::ast::Stmt)]` ＋ `Stmt` が
  `visitor!(...)` に）は `visit_stmt` に下がる；辿られるが非列挙 ⇒ ドリル；`#[subast]` に無い ⇒ 静かに
  無視（`_` に束縛）；import/エイリアスのフィールド
  （`use other::Stmt; … s: Stmt` ＋ `#[subast(other::Stmt)]`）は解決する
  （`core/tests/visitor_local_types.rs` が記録するギャップ）；末尾セグメントが衝突する `#[subast]`
  （`a::Foo`, `b::Foo`）は derive で失敗する。
- 自己再帰: `Expr { Bin(Box<Expr<…>>, …) }`（`Expr` は列挙だが自分の `#[subast]` には*入れない*）は
  `visit_expr` メソッド経由で再帰する（自身のスクルティニーは `@SELF`）。
- 循環ガード: *未列挙*中間型の循環（`Cast → Cast`、どれも `visitor!(...)` に無い）⇒ `__visitor_build`
  エラー；どれか 1 つを列挙すれば解消。visited 型に達せず有限でリーフに底をつく未列挙中間型 ⇒ `visit_*`
  呼び出し無し、エラー無し。
- コンテナ: `Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` フィールド（ヘッドが `#[subast]` に）を持つ型は
  各要素に潜る（visited ⇒ メソッド、未列挙 ⇒ ドリル）；reduce/append は親の `visit_*_mut` の
  オーバーライドで。

# TODOs

- [ ] `#[derive(Ast)]` の出力で leaker 型を定義せず、代わりにマクロ対象に直接 Repeater を実装する。
