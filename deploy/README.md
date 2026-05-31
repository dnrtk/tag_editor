# Tag Editor — OpenMediaVault(Raspberry Pi) への Docker デプロイ手順

デスクトップアプリに内蔵された Web サーバ部分だけ（GUI なし）を、OMV 上の
Docker コンテナとして常駐させる手順です。ビルドは `--no-default-features` で
GUI 依存（eframe/winit/GTK）を外した pure-Rust のサーバのみになります。

OMV は標準では Docker を持たないため、**omv-extras → Compose プラグイン**を
入れて有効化します。途中で一度シェルが要るので、まず SSH を有効化します。

---

## 1. SSH を有効化する（Web UI から）

OMV 管理画面 → **Services → SSH** → *Enable* にチェック → *Save / Apply*。
これで `ssh <ユーザー>@<NASのIP>` でログインできます（イメージの取り込みに使用）。

## 2. omv-extras を導入する（SSH で1行）

```sh
sudo wget -O - https://github.com/OpenMediaVault-Plugin-Developers/installScript/raw/master/install | sudo bash
```

実行後、OMV 管理画面を再読み込みすると **System → omv-extras** が現れます。

## 3. Docker と Compose プラグインを有効化する（Web UI）

- OMV 管理画面 → **System → omv-extras → Docker** タブで Docker エンジンを
  インストール（データ保存先は SSD/HDD 上の共有を推奨。SD カードは消耗します）。
- **System → Plugins** で `openmediavault-compose` を選択してインストール。
- **Services → Compose → Settings** で Docker のパス等を確認・適用。

## 4. CPU アーキを確認する（ターゲット決定）

```sh
uname -m
#   aarch64 → 64bit → linux/arm64
#   armv7l  → 32bit → linux/arm/v7
```

## 5. イメージをビルドして OMV に取り込む

**PC（buildx 利用・推奨）でビルドし、tar で持ち込む:**

```sh
# リポジトリのルートで（アーキに合わせてどちらか）
docker buildx build --platform linux/arm64  -t tag-editor:latest --load .
docker buildx build --platform linux/arm/v7 -t tag-editor:latest --load .

docker save tag-editor:latest -o tag-editor.tar
# tag-editor.tar を SMB 共有などで NAS にコピーし、SSH で取り込む:
docker load -i /srv/<あなたの共有>/tag-editor.tar
```

> Pi 上で直接ビルドする場合は、ソース一式を NAS に置いて
> `docker build -t tag-editor:latest .`（Pi3 では10〜20分程度）。

## 6. compose と settings.json を用意する

このフォルダの `docker-compose.yml` と `settings.json`（`settings.json.example`
をコピー）を、Compose プラグインが管理するスタック用フォルダに置きます
（Compose プラグインの *Files* で新規スタックを作り内容を貼り付けるか、SSH で配置）。

編集ポイント:

- `docker-compose.yml` の **volume**: NAS の写真フォルダを `/data` にマッピング
  （例 `/srv/dev-disk-by-uuid-XXXX/photos:/data`）。タグは XMP として画像へ
  書き込むので **読み書き可**でマウントすること。
- `settings.json` の `shared_folders[].path` は**コンテナ内パス（`/data`）**を指定。
  これがアクセス許可リスト兼ブラウズ対象になります（ホスト側パスではない点に注意）。
- `web_port` と `hotkey_tags` も必要に応じて編集。

## 7. 起動する

Compose プラグインの該当スタックで **up**（または SSH で
`docker compose up -d`）。`restart: unless-stopped` なので再起動後も自動復帰します。

## 8. アクセス

LAN 内の端末から `http://<NASのIP>:47823/` を開く。左サイドバーの
「Shared folders」に登録フォルダが出るので、選んでタグ付けを始められます。

> ファイアウォール: OMV は通常 LAN 向けの公開ポートを塞ぎませんが、
> 繋がらない場合は当該ポート(TCP)の受信を許可してください。

---

## なぜ k8s(k3s) ではなく Docker か

単一コンテナを1個動かすだけなら、k3s は Deployment/Service/PVC のマニフェストが
必要で管理が煩雑なうえ、k3s 自体が Pi3 (RAM 1GB) には重めです。Compose の方が
軽量で UI 管理も簡単なため、本構成では Docker(Compose) を推奨します。

## 補足

- タグは画像ファイル内（XMP）に保存されるため、NAS の共有に直接反映されます。
- アクセスは `shared_folders` とその配下に限定され、範囲外のパスは 403 になります。
- `./cache` ボリュームはタグキャッシュを永続化し、再起動後も再スキャンを高速に保ちます（任意）。
