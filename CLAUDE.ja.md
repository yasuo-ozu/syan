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

# Drill-in 実装計画（結論: 推移的に自動探索するビジター）

## 背景

元々の「drill-in」の目的は、ビジターが `visit_*` を持たない Ast 型を*通り抜けて*走査することだった
（仕様の `Expr::Cast(cast) => this.visit_type(&cast.0)`）。下記の深掘りにより、これはより単純かつ強力な
ものに帰着する — **到達可能な AST の閉包を自動探索し、発見した全ての型に `visit_*` メソッドを与える**
（*モデル* と *解決済みの決定* を参照）。よって独立した「drill」経路は不要になる。これを阻んでいたのは
「このフィールドのヘッドは辿るべき `Ast` 型か？」をマクロ展開時に判定できないことだった。
**`#[subast(...)]` アローリスト**がそれを取り除く: `#[derive(Ast)]` の struct/enum は自分のサブ AST
型を（解決可能なパス付きで）型レベル `#[subast(...)]` に宣言する；**フィールドは、その（コンテナを剥がした）
ヘッドがそこに列挙されているとき**（または型自身の型 — 自己再帰は暗黙）**に限り辿られる**。**それ以外の
フィールド型はすべて無視される**（リーフ扱い: トークン、プリミティブ、`PhantomData`、span 等）。1 つの
属性が両方の役割を担う: メンバシップ（列挙 ⇒ 辿る）*と*パス解決（その entry がサブ AST のメタデータ
マクロへの解決可能なパスを供給）。これは（`#[derive(Ast)]` が既に出している）`Repeater` impl に
フォールバックの消費者を与える: ポータブルなサブ AST 参照。

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

## モデル — visit-all-reachable（「drill-in」が帰着する先）

`visitor!(Root, …)` は**エントリ型**を列挙する；`__visitor_build` は `#[subast]` の辺（＋暗黙の自己再帰）
を辿って到達可能な閉包を自動探索し、**発見した全ての型**に `visit_*` メソッドを生成する（syn スタイル）。
走査は常に `this.visit_<head>(field)` — *メソッド*呼び出しなので、再帰はトレイト経由で回り、再帰的/循環的な
AST も現状の visit 対象型の走査と全く同様に扱える。**インラインの drill も循環検出も無い**: `Cast`（親の
`#[subast]` に列挙）のようなラッパは単に*こちらも visit される*（`visit_cast` が生成され、その本体が
`Type` に到達する）。これは仕様の「不可視な通り抜け」より一様で、「依存集合全体を列挙せずにビジターを構築
する」に合致する。サブツリーを枝刈りするには、そのヘッドを `#[subast]` から外す。

## `__visitor_build`（`macro/visitor.rs`）

- **自動探索 ＋ 逐次生成**（*スケール* 参照）: ping-pong の各バウンスで 1 つの型の構造を取得し、*その型の*
  フリー関数 `visit_*` / `visit_*_mut` を即座に出力し、その name/path/own-generics を持ち回す
  name-list に記録し、辿られる各フィールドの**解決済みパス**をキューに積む — **ただし `@SELF` は除く**
  （それは現在の型で、既に記録済み。自己再帰はメソッド呼び出しであって新たな取得ではない） —
  パスの末尾セグメントの**文字列**で重複排除（proc-macro の文字列比較なので可）、そして構造は捨てる。
- **本体のローワリング**（フィールドごと；コンテナを剥がして内側ヘッドを visit）:
  - **無視**フィールド（`@leaf` — ヘッドが `#[subast]` に無く、自己でもない）⇒ `_` に束縛してスキップ。
  - **辿る**フィールド ⇒ `this.visit_<inner-head>(<access expr>)` — メソッド名はリテラルのヘッド ident
    から proc-macro 内で構築（`to_snake`；`macro_rules!` では決して作らない）、access expr はコンテナの
    ローワリングを適用、enqueue パスと enum の match スクルティニーはレコードの**解決済みパス**を使う
    （`#[subast]` entry のパス、または `@SELF`；`path_of` も推論も無し）。全ての辿るヘッドはメソッドを
    持つ発見済みの型なので、**ローワリング時にメンバシップ判定は不要**。
- **1 回限りの項目**（最終バウンス、name-list から — シグネチャのみ、構造は不要）: `Visit`/`VisitMut`
  トレイト（発見した型ごとに 1 メソッド）、`Driver`/`Hook`/`Chain`、`IntoVisitor`/`IntoVisitorMut` の
  クロージャ＆タプル impl、インヒーレント `visit`/`visit_mut`。

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

### Decision 2 — 委譲とモデル: **proc-macro が組み立てる；visit-all-reachable。**
2 つの厳然たる `macro_rules!` の事実が決め手: (1) 2 つの ident の等価**比較ができない**（`$a == $b` は
無く、matcher 内でメタ変数名を再利用するとエラー）→ visit 集合のメンバシップ判定ができない；(2) ident の
**連結/snake_case 化ができない**（`format_ident!` が無い）→ `Stmt` から `visit_stmt` を作れない。よって
メンバシップ*も*本体生成（メソッド名、match アーム）*も* proc-macro 側に置くしかない。「`macro_rules!`
で委譲」は、各型の構造をメタデータマクロが*供給する*こととして実現される（ping-pong の取得そのものが
委譲）；`__visitor_build` が組み立てる。proc-macro が組み立てるので、選択的 drill ではなく
**visit-all-reachable** を採る — インライン drill の再帰と循環処理が不要になる（再帰はメソッド呼び出し
経由）。`#[subast]` が唯一の**アローリスト**: メンバシップを宣言し（列挙 ⇒ 辿る；それ以外は無視）*かつ*
解決可能パスを供給する。「到達可能」とは `#[subast]` の辺（＋自己）を通って到達可能の意で、そうした型は
すべてメソッドを得る。（メンバシップは `macro_rules!` の判定ではなく `#[subast]` に宿る — 事実 (1) と整合。）

### スケール — 逐次生成（O(N²) を回避）。
完全な AST の閉包は ~100 型になりうる；取得した構造を全て ping-pong の状態に溜め、各バウンスで再出力すると
O(N²) トークンになる。代わりに、構造は**バウンスごとに使い捨て**にし、各型の `visit_*` 関数は**取得した
その場で出力**する；溜まるのは小さな **name-list**（ident ＋ path ＋ own-generics）だけ（Rust は項目の
順序が自由なので、本体が最後に出力されるトレイトを参照してよい）。トレイト / `Driver` / `IntoVisitor` /
インヒーレント項目はその name-list から最後に一度だけ出力する。トレイトの**和集合ジェネリクス ＝ ルート型
のジェネリックパラメータ**（最初に取得した型から判明し、本体を出す前に分かるのでクロージャも動く）；新たな
ジェネリックパラメータ名を導入するサブ型はエラー。

### パス解決 — `#[subast]` が辿る全てのパスを供給する。
辿られる各フィールドの解決可能パスは、一致する `#[subast]` entry のパス（または自己再帰の `@SELF` —
その型を取得したパスで置換され、match スクルティニーにのみ使われ、探索の辺としては決して enqueue されない）。
未列挙のフィールド型は無視されるので、それらに推論すべきパスは無い。`#[subast]` のパスは*定義元*モジュールで
解決され、ポータブルに再公開される — 1 つの `pub use` が**型とメタデータマクロの両名前空間**を運び、
`#[macro_export]` 側を `$crate` 根付けすることで同一クレートでも下流でも解決する。モジュール前置の推論も
兄弟の当て推量も無い: entry の欠落/打ち間違いは*静かに無視される*フィールド（指示が受け入れた失敗モード）で
あり、`unused entry` 警告 / 「何も辿らない」リントでしか捕まらず、誤解決のパスにはならない。残る穴:
`visitor!(...)` の*エントリ*パス自体が非正準な再エクスポートのときにサブ AST の*型*を名指す件 —
`visitor!(...)` のエントリパスを正準（`crate::`/`super::` 根付け）に要求することで塞ぐ（これにより、継続中の
TODO どおり `Leaker` を落とせる）。

## テスト（`core/tests`）

- 仕様グラフ: `Type`、`Cast(Type)`、`Expr { Cast(Cast) }`（`Expr` に `#[subast(super::Cast)]`、`Cast` に
  `#[subast(super::Type)]`）；`visitor!(super::Expr)` ⇒ 走査は `Cast` を通って `Type` に到達する
  （自動生成された `visit_cast` → `visit_type`）；クロージャ `|t: &Type<()>| …` が 1 回発火する；`Cast`
  も visit 可能（`visit_cast` が存在する — visit-all の帰結）。
- アローリスト: ヘッドが列挙されたフィールド（`#[subast(crate::ast::Stmt)]`、フィールド
  `Box<Stmt<S>>`）は辿られる（`visit_stmt`）；**未列挙**のフィールド型は静かに無視される（`_` に束縛、
  走査しない）；import/エイリアスのフィールド（`use other::Stmt; … s: Stmt` ＋ `#[subast(other::Stmt)]`）
  は `visit_stmt` に到達する（`core/tests/visitor_local_types.rs` が記録するギャップ）；末尾セグメントが
  衝突する `#[subast(a::Foo, b::Foo)]` は derive で局所的なエラーになる。
- 自己再帰: `Expr { Bin(Box<Expr<…>>, …) }` は `Expr` を自分の `#[subast]` に*入れなくても* `@SELF`
  経由で再帰する。
- コンテナ: `Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` フィールドを持つ型（`Stmt`/`Expr` を列挙済み）は
  各要素に潜る；reduce/append は親の `visit_*_mut` のオーバーライドで。
- 自動探索のスケール: 複数型グラフ上で `visitor!(super::Root)` ⇒ `#[subast]` の辺を通って到達可能な型は
  全てメソッドを得て走査される。

# TODOs

- [ ] `#[derive(Ast)]` の出力で leaker 型を定義せず、代わりにマクロ対象に直接 Repeater を実装する。
