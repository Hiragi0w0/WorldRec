# WorldRec

[![License: GPL-3.0-only](https://img.shields.io/badge/License-GPL--3.0--only-blue.svg)](./LICENSE)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows-0078D6.svg)](#動作環境)

**WorldRec** は、VRChat のワールド訪問履歴を自動で記録・可視化するデスクトップアプリです。
VRChat のログを監視して訪問履歴を SQLite に蓄積し、日付レールやタイムラインで振り返れます。
VRChat API 連携でワールド情報やサムネイルを取得し、Gemini による AI 推薦も利用できます。

> **ステータス: ベータ版（0.1.x）**
> 現在はベータ版です。仕様やデータ構造は今後変更される可能性があります。不具合や要望は [Issue](../../issues) でお知らせください。

Svelte 5 + TypeScript + Vite でフロントを構成し、Rust/Tauri command 経由で SQLite の訪問履歴を読み込みます。
AI 推薦は Go 製サイドカー（`worldrec-ai`）が担当します。

## 主な機能

- **訪問履歴の自動記録** — VRChat のログを監視し、訪問ワールド・滞在時間を SQLite に保存。
- **履歴の可視化** — 日付レール／タイムラインで訪問履歴を閲覧。
- **ワールド情報の取得** — VRChat API からワールド名・作者・サムネイル等を取得。
- **AI 推薦** — Gemini（サイドカー `worldrec-ai`）による、履歴に基づくワールド推薦。
- **セキュアな認証情報管理** — API キーは OS のキーリング（資格情報マネージャー）に保存。

## 主な画面

- `RecordView`: 訪問履歴、日付レール、タイムライン、DB/ログ監視状態を表示します。
- `RuntimeStatusCard`: 実際に参照しているDBパス、VRChatログディレクトリ、監視状態、DB内の訪問履歴件数、最新訪問日時を表示します。
- `VisitTimeline`: 選択中の日付または表示条件に一致する訪問履歴を表示します。

## 動作環境

- **OS**: Windows 10 / 11（現状 Windows を主対象としています）
- **WebView2 ランタイム**: Tauri アプリの実行に必要です。多くの Windows 環境には標準で導入されていますが、無い場合はインストーラーが自動で導入します。

## インストール（ベータ利用者向け）

1. [Releases](../../releases) ページから最新のインストーラー（`.msi` または `.exe`）をダウンロードします。
2. インストーラーを実行します。
   - 現在のベータ版は **未署名** のため、初回起動時に Windows SmartScreen の警告が表示される場合があります。
     その場合は「詳細情報」→「実行」で続行できます。
3. アプリを起動すると、VRChat のログ監視が始まり、訪問履歴が記録されていきます。

## データとプライバシー

- VRChat のログを読み取り、訪問履歴を SQLite（下記パス）に **ローカル保存** します。
- API キーは平文でファイルに残さず、OS のキーリングに保存します。
- AI 推薦機能を使ったときのみ、ワールド情報などが Gemini API（Google）へ送信されます。

詳細は [SECURITY.md](./SECURITY.md) を参照してください。

### 実データの参照先

DBパスが未設定の場合、Tauri側は次の既定パスを参照します。

```text
%LOCALAPPDATA%\WorldRec\worldrec.db
```

アプリ上では `RuntimeStatusCard` の `DB path` と `訪問履歴 N 件` で、参照先と件数を確認できます。

設定画面で DB 保存先または VRChat ログフォルダを変更すると、指定先を検証してからログ監視を停止し、変更前に監視中だった場合だけ新しい保存先で再開します。新しい保存先で再開できない場合は以前の設定と監視先への復旧を試みます。DB を切り替えても履歴の自動コピー・移動・削除は行わないため、元の履歴を表示するには元の DB パスへ戻してください。

### 履歴が空に見える場合

履歴が表示されない場合でも、DB自体が空とは限りません。現行UIでは次の状態を分けて表示します。

- 読み込み中
- 読み込み失敗
- DBには接続できたが履歴が0件
- 現在の表示条件に一致する履歴が0件
- 今日または昨日の履歴は0件だが、DB全体には履歴がある

DB全体に履歴があるのに現在の条件で0件になっている場合は、空状態カードに `全件表示に切り替える` ボタンが表示されます。

## 開発

### 前提ツール

| ツール | バージョン | 用途 |
|--------|-----------|------|
| Node.js | 22 以上 | フロントエンド（Svelte + Vite） |
| Rust | 1.77.2 以上（stable 推奨） | Tauri バックエンド |
| Go | 1.24 以上 | AI サイドカー（`worldrec-ai`） |

Tauri の OS 別前提条件は [公式ドキュメント](https://tauri.app/start/prerequisites/) を参照してください。

### セットアップ

```powershell
npm install
```

### 開発時のブラウザ確認

通常のブラウザで Vite dev server を開く場合、Tauri IPC は使えません。そのため開発時は `vite.config.ts` の dev bridge が `__worldrec_dev` エンドポイントでローカルDBを読みます。

```powershell
npm run dev -- --host 127.0.0.1
```

Tauri アプリとして開発起動する場合:

```powershell
npm run tauri -- dev
```

厳密にDBを読み取り専用で確認したい場合は、アプリ経由ではなく SQLite の read-only 接続で確認してください。

```powershell
python -c "import os, sqlite3; p=os.path.join(os.environ['LOCALAPPDATA'], 'WorldRec', 'worldrec.db'); c=sqlite3.connect('file:'+p+'?mode=ro', uri=True); print(c.execute('SELECT COUNT(*) FROM visit_histories').fetchone()[0]); c.close()"
```

### 検証・ビルドコマンド

フロントエンドの型検査:

```powershell
npm run check
```

フロントエンドのビルド:

```powershell
npm run build
```

AI サイドカーのビルド（`src-tauri/binaries/worldrec-ai-<triple>` を生成）:

```powershell
npm run build:sidecar
```

Tauriアプリのrelease用ビルド:

```powershell
npm run tauri -- build
```

`src-tauri/tauri.conf.json` では `bundle.active` が `true`、`bundle.targets` が `all` です。Tauriのreleaseビルドでは `beforeBuildCommand` により `npm run build:sidecar` と `npm run build` も実行され、生成物は `src-tauri\target\release\` と `src-tauri\target\release\bundle\` 配下に出力されます。

Rust側のテスト:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Go サイドカーのテスト:

```powershell
cd sidecar
go vet ./...
go test ./...
```

## 実装メモ

- `src-tauri/src/commands/mod.rs::list_visits` が訪問履歴取得のTauri commandです。
- `recent` は既定で最大100件、`all` は全件表示用に `LIMIT` を付けません。
- `src/lib/state/histories.svelte.ts` は最後に読み込んだ `VisitFilterCriteria` を保持し、UIがフィルタ0件とDB 0件を区別できるようにします。
- `src/lib/components/views/RecordView.svelte` が空状態の文言を決定し、`src/lib/components/history/VisitTimeline.svelte` が空状態カードと全件表示ボタンを描画します。

## License

WorldRec is licensed under the GNU General Public License v3.0 only (`GPL-3.0-only`).

See [LICENSE](./LICENSE) for details.
