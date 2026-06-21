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
ものに帰着する — **到達可能な AST の閉包を自動探索し、全ての型に `visit_*` メソッドを与える**（*モデル*
と *解決済みの決定* を参照）。よって独立した「drill」経路は不要になる。これを阻んでいたのは「この
フィールドのヘッドは `Ast` 型か？」をマクロ展開時に判定できないことだった。**`#[no_ast]` 規約**がそれを
取り除く: `#[derive(Ast)]` の struct/enum において、**各フィールドの（ヘッド）型もまた
`#[derive(Ast)]`**である（= 名前で到達可能なメタデータマクロを持つ）。ただしフィールドが `#[no_ast]`
の場合を除く（リーフ: `Token![..]`、`Integer`、プリミティブ、`PhantomData`、span 等）。ジェネレータは
Ast かどうかを判定しない: `#[no_ast]` でなければ規約上 Ast；`#[no_ast]` の付け忘れは「cannot find macro
`Foo`」という明確なエラーになる。仕事は 2 つの直交する軸で行う: **`#[no_ast]`** が分類（Ast かリーフか、
ディナイリスト）を担い、補完的な型レベル属性 **`#[subast(...)]`** がパス解決（visit 側のコンテキストから
辿ったヘッドのマクロに到達すること）を担う — `#[subast]` はメンバシップを一切変えない。これは
（`#[derive(Ast)]` が既に出している）`Repeater` impl にフォールバックの消費者を与える: ポータブルな
サブ AST 参照。

## `#[derive(Ast)]`（`macro/ast.rs`）

2 つの直交する軸: **`#[no_ast]`** が*分類*（Ast かリーフか）を、**`#[subast]`** が*パス解決*を担う。
ヘルパー属性: `#[proc_macro_derive(Ast, attributes(syan, no_ast, subast))]`。

- **`#[no_ast]`**（ディナイリスト） — **フィールド**単位ではリーフを示す（`_` に束縛し、辿らない）；
  **型**単位では型全体をリーフにする（`visit_*` を生成せず、潜らない — 例えば全変種が `Token!` の
  `BinOp` を 1 つの印で）。`#[no_ast]` でないものはすべて Ast の子で、辿られる。「cannot find macro
  `Foo`」という*声の大きい*失敗が付け忘れの安全網であり、アローリストの*静かな*取りこぼしより厳密に安全
  （だから分類はアローリスト `#[subast]` ではなくディナイリストにする）。
- **`#[subast(<paths>)]`**（パス解決であり、メンバシップ**ではない**） — 辿られるヘッドへの解決可能な
  パスの型レベル辞書。**定義元のモジュール**で解決される（形式: `crate::` / `::abs::` / `super::` /
  `self::` / 同一モジュールの裸の兄弟 / `b::Foo as BFoo`；ジェネリック引数は書かない）。辿られるヘッドが
  *どこにあるか*を derive に教えるだけで、visit 閉包への追加/削除は一切しない。**自分自身は決して列挙
  しない**（列挙すると derive がエラーにする — 自己参照は下記 `@SELF` で解決）。
- **デフォルトのパス規則**（辿られる各フィールドにつき、コンテナを剥がして内側ヘッドへ；§Decision 1）、
  優先順: (1) フィールド自身のパスが**複数セグメント**なら（`Vec<crate::ast::Pat<S>>` ⇒
  `crate::ast::Pat`）そのまま使う（よって明示的にパスを書いたフィールドの多くは entry 不要）；(2) でなく
  裸のヘッドなら（末尾セグメントで、エイリアスを考慮して）`#[subast]` の entry に一致させ、その entry の
  パスを使う；(3) でなく裸のヘッドなら、同一モジュールの**兄弟**として扱い、定義元モジュールに derive が
  生成するポータブルな再エクスポート `#[doc(hidden)] pub use Head as __subast_…;` を出す（1 つの
  `pub use` が**型とそのマクロ `Head!` の両方**を再公開する）。実際には兄弟でない裸のヘッド（private な
  `use` で入ってきて未列挙）は、その `pub use` を**フィールドの位置・定義元モジュールで**失敗させる —
  明確で局所的なエラー。よって `#[subast]` が参照されるのは段階 (2) だけ: 裸でクロスモジュール/エイリアス
  の残余ケース。同一モジュールの兄弟や明示パスのフィールドは何も要らない。
- メタデータマクロは variant → field ごとに明示レコードを出す: **アクセサ**（タプル添字 / 名前付き
  ident）、**コンテナ**（`direct`/`box`/`vec`/`vecdeque`/`option`/`slice`/`punctuated`）、内側の
  **ヘッド ident**（リテラル — proc-macro 側で `visit_<snake(head)>` を構築するため。`macro_rules!`
  では*決して*作らない）、そして**解決済みパス**（上記の規則；自己参照ヘッドは `@SELF` とし、
  `__visitor_build` が「その型を取得したパス」で置換する）。`#[no_ast]` フィールドは `@no_ast` だけを
  持つ。derive は定義サイトで診断を行う: `#[subast]` entry 間の末尾セグメント衝突（エラー）、どの
  フィールドにも一致しない entry（警告）。
- `Leaker` ＋ `Repeater<N>` impl は最終手段の型ネーマ / 外部メタデータ消費者向けに維持する；ビジター
  経路は今や `#[subast]` で解決したパスを使う。

## モデル — visit-all-reachable（「drill-in」が帰着する先）

`visitor!(Root, …)` は**エントリ型**を列挙する；`__visitor_build` は非 `#[no_ast]` フィールドを辿って
到達可能な閉包を自動探索し、**発見した全ての型**に `visit_*` メソッドを生成する（syn スタイル）。走査は
常に `this.visit_<head>(field)` — *メソッド*呼び出しなので、再帰はトレイト経由で回り、再帰的/循環的な
AST も現状の visit 対象型の走査と全く同様に扱える。**インラインの drill も循環検出も無い**: `Cast` の
ようなラッパは単に*こちらも visit される*（`visit_cast` が生成され、その本体が `Type` に到達する）。これは
仕様の「不可視な通り抜け」より一様で、「依存集合全体を列挙せずにビジターを構築する」に合致する。サブツリーを
枝刈りするには、そこへ至るフィールドに `#[no_ast]` を付ける。

## `__visitor_build`（`macro/visitor.rs`）

- **自動探索 ＋ 逐次生成**（*スケール* 参照）: ping-pong の各バウンスで 1 つの型の構造を取得し、*その型の*
  フリー関数 `visit_*` / `visit_*_mut` を即座に出力し、その name/path/own-generics を持ち回す
  name-list に記録し、辿られる各フィールドの**解決済みパス**をキューに積み（パスの末尾セグメントの
  **文字列**で重複排除 — proc-macro の文字列比較なので可）、構造は捨てる。
- **本体のローワリング**（フィールドごと；コンテナを剥がして内側ヘッドを visit）:
  - `#[no_ast]` ⇒ `_` に束縛してスキップ。
  - それ以外 ⇒ `this.visit_<inner-head>(<access expr>)` — メソッド名はリテラルのヘッド ident から
    proc-macro 内で構築（`to_snake`；`macro_rules!` では決して作らない）、access expr はコンテナの
    ローワリングを適用、enqueue パスと enum の match スクルティニーはレコードの**解決済みパス**を使う
    （`path_of` も推論も無し）。全ての内側ヘッドはメソッドを持つ発見済みの型なので、**メンバシップ判定は
    不要**。
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
ヘッドは `#[no_ast]` 規約により Ast（リーフのコンテナ、例えば `Vec<Token>` は `#[no_ast]`）。reduce/append
は不変: 親の `visit_*_mut`（`&mut Vec` / `&mut Option` を所有）をオーバーライドする。

### Decision 2 — 委譲とモデル: **proc-macro が組み立てる；visit-all-reachable。**
2 つの厳然たる `macro_rules!` の事実が決め手: (1) 2 つの ident の等価**比較ができない**（`$a == $b` は
無く、matcher 内でメタ変数名を再利用するとエラー）→ visit 集合のメンバシップ判定ができない；(2) ident の
**連結/snake_case 化ができない**（`format_ident!` が無い）→ `Stmt` から `visit_stmt` を作れない。よって
メンバシップ*も*本体生成（メソッド名、match アーム）*も* proc-macro 側に置くしかない。「`macro_rules!`
で委譲」は、各型の構造をメタデータマクロが*供給する*こととして実現される（ping-pong の取得そのものが
委譲）；`__visitor_build` が組み立てる。proc-macro が組み立てるので、選択的 drill ではなく
**visit-all-reachable** を採る — インライン drill の再帰と循環処理が不要になる（再帰はメソッド呼び出し
経由）。`#[subast]` はここに*パス辞書*として収まる（アローリストではない）: 解決可能なパスを供給するが
メンバシップは宣言しない（それは `#[no_ast]` のみが担う）ので、visit-all-reachable は保たれる。

### スケール — 逐次生成（O(N²) を回避）。
完全な AST の閉包は ~100 型になりうる；取得した構造を全て ping-pong の状態に溜め、各バウンスで再出力すると
O(N²) トークンになる。代わりに、構造は**バウンスごとに使い捨て**にし、各型の `visit_*` 関数は**取得した
その場で出力**する；溜まるのは小さな **name-list**（ident ＋ path ＋ own-generics）だけ（Rust は項目の
順序が自由なので、本体が最後に出力されるトレイトを参照してよい）。トレイト / `Driver` / `IntoVisitor` /
インヒーレント項目はその name-list から最後に一度だけ出力する。トレイトの**和集合ジェネリクス ＝ ルート型
のジェネリックパラメータ**（最初に取得した型から判明し、本体を出す前に分かるのでクロージャも動く）；新たな
ジェネリックパラメータ名を導入するサブ型はエラー。

### パス解決 — `#[subast]` 辞書（モジュール前置の推論を置き換える）。
辿られるヘッドの解決可能パスは**デフォルトのパス規則**（§`#[derive(Ast)]`）から得る: フィールド自身の
複数セグメントパスをそのまま、でなければ裸のヘッドを（エイリアス考慮で）`#[subast]` に一致、でなければ
derive が出す同一モジュール兄弟の再エクスポート。`#[subast]` 辞書は*定義元*モジュールで解決され、
ポータブルに再公開される — 1 つの `pub use` が**型とメタデータマクロの両名前空間**を運び、
`#[macro_export]` 側を `$crate` 根付けすることで同一クレートでも下流でも解決する。旧来の「定義元モジュール
を前置する」推論は撤去（import/エイリアスのヘッドを静かに誤解決していた）；今やエラーはフィールドの位置・
定義元モジュールで表面化し、下流の「cannot find macro」にはならない。残る穴: `visitor!(...)` の*エントリ*
パス自体が非正準な再エクスポートのときにサブ AST の*型*を名指す件 — `visitor!(...)` のエントリパスを正準
（`crate::`/`super::` 根付け）に要求することで塞ぐ（これにより、継続中の TODO どおり `Leaker` を落とせる）。

## テスト（`core/tests`）

- 仕様グラフ: `Type`、`Cast(Type)`、`Expr { Cast(Cast) }`；`visitor!(super::Expr)`（エントリのみ列挙）
  ⇒ 走査は `Cast` を通って `Type` に到達する（自動生成された `visit_cast` → `visit_type` 経由）；
  クロージャ `|t: &Type<()>| …` が 1 回発火する；`Cast` も visit 可能（`visit_cast` が存在する —
  visit-all の帰結）。
- コンテナ: `Vec<Stmt>` / `Option<Expr>` / `Box<Expr>` フィールドを持つ型は各要素に潜る；reduce/append は
  親の `visit_*_mut` のオーバーライドで。
- `#[no_ast]`: リーフ/リーフのコンテナのフィールドはスキップ；型レベル `#[no_ast]`（例えば全変種が
  `Token!` の `BinOp`）は `visit_*` を生成せず潜らない；`#[no_ast]` の無い非 Ast フィールドは定義サイト
  （兄弟再エクスポートのエラー）か、下流の「cannot find macro」で失敗する。
- `#[subast]`: import/エイリアスのフィールド（`use other::Stmt; … s: Stmt` ＋ `#[subast(other::Stmt)]`）
  ⇒ `visit_*` が `visit_stmt` に到達する（`core/tests/visitor_local_types.rs` が記録するギャップ）；
  末尾セグメントが衝突する `#[subast]`（`a::Foo`, `b::Foo`）は derive で局所的なエラーになる。
- 自動探索のスケール: 複数型グラフ上で `visitor!(super::Root)` ⇒ 到達可能な非リーフ型は全てメソッドを得て
  走査される。

# TODOs

- [ ] `#[derive(Ast)]` の出力で leaker 型を定義せず、代わりにマクロ対象に直接 Repeater を実装する。
