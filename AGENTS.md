# AGENTS.md: dood

## ADR

決定は `docs/adr/` に1決定1ファイルで記録する。書式は `docs/adr/0000-template.md`。

**機能実装・ライブラリ選定・リファクタなど、意思決定の前に `docs/adr/` を読む。** `Accepted` が現在有効な決定で、作業はこれに従う。`Rejected` / `Superseded` / `Deprecated` は経緯として読むだけで、採用しない。`Accepted` と矛盾させる必要が出たら、進める前にユーザーに確認する。

### ADRを書く（依頼されたとき、または決定を残すと決めたとき）:

1. `docs/adr/` の最大番号+1で `docs/adr/XXXX-kebab-case-title.md` を作る（`0000` は雛形用なので `0001` から）
2. `docs/adr/0000-template.md` に沿って書く（`status` / `date`、Context / Decision / Consequences）
3. 過去の決定を覆す場合は、古いADRの `status` を `Superseded` にし、新しいADRを指す

