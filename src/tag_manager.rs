use little_exif::metadata::Metadata;
use little_exif::exif_tag::ExifTag;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// タグを読み込む (Exif UserCommentから)
pub fn load_tags(image_path: &Path) -> Vec<String> {
    if !is_supported_format(image_path) {
        return Vec::new();
    }

    // メタデータ読み込み
    if let Ok(metadata) = Metadata::new_from_path(image_path) {
        // UserCommentを探す
        // Note: little_exifのget_tag引数は検索用のダミーインスタンスが必要な場合がある
        // バージョンによって異なるが、一般的にTag Variantを渡す
        
        // UserComment (0x9286)
        if let Some(tag) = metadata.get_tag(&ExifTag::UserComment(Vec::new())).next() {
            if let ExifTag::UserComment(data) = tag {
                // データの先頭に文字コード識別子がある場合とない場合がある
                // ASCII\0\0\0 または UNICODE\0 など
                // ここでは単純にUTF8文字列としてパースを試みる
                
                let s = String::from_utf8_lossy(data);
                let content = s.trim();
                
                // "ASCII\0\0\0" などを除去
                let clean_content = if content.starts_with("ASCII") {
                    &content[8..] 
                } else if content.starts_with("UNICODE") {
                     &content[8..]
                } else {
                    content
                };
                
                // ヌル文字が含まれている場合があるので除去
                let clean_content = clean_content.trim_matches(char::from(0));

                return clean_content
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    
    Vec::new()
}

/// タグを保存する (Exif UserCommentへ)
pub fn save_tags(image_path: &Path, tags: &[String]) -> std::io::Result<()> {
    if !is_supported_format(image_path) {
        return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Unsupported format"));
    }

    // 既存のメタデータを読み込むか、新規作成
    let mut metadata = Metadata::new_from_path(image_path).unwrap_or_else(|_| Metadata::new());
    
    // タグをセミコロン区切りで結合
    let content = tags.join(";");
    
    // UserCommentとして設定
    // Exif規格では "ASCII\0\0\0" + content が一般的だが、
    // 最近のリーダーはUTF-8をそのまま読めることも多い。
    // 安全のため、純粋な文字列バッファとして書き込む
    
    // little_exifは自動でヘッダをつけないので、自分でバリデーションが必要だが
    // ここではシンプルにバイト列として保存する
    metadata.set_tag(ExifTag::UserComment(content.into_bytes()));

    // ファイルに書き込む
    // little_exifのwrite_to_fileは既存ファイルを上書き保存する
    metadata.write_to_file(image_path)
}

/// タグの追加
pub fn add_tag(tags: &mut Vec<String>, tag: &str) {
    let tag = tag.trim().to_string();
    if !tag.is_empty() && !tags.contains(&tag) {
        tags.push(tag);
    }
}

/// タグの削除
pub fn remove_tag(tags: &mut Vec<String>, tag: &str) {
    tags.retain(|t| t != tag);
}

/// タグのトグル（存在すれば削除、なければ追加）
pub fn toggle_tag(tags: &mut Vec<String>, tag: &str) -> bool {
    let tag = tag.trim().to_string();
    if tags.contains(&tag) {
        remove_tag(tags, &tag);
        false
    } else {
        add_tag(tags, &tag);
        true
    }
}

/// ディレクトリ内の全画像からタグを収集
pub fn collect_all_tags(dir: &Path) -> HashSet<String> {
    let mut all_tags = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_supported_format(&path) {
                for tag in load_tags(&path) {
                    all_tags.insert(tag);
                }
            }
        }
    }
    all_tags
}

/// メタデータ埋め込みに対応しているフォーマットか判定
pub fn is_supported_format(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp"
        )
    } else {
        false
    }
}

/// ファイルが画像かどうかを判定 (表示用)
pub fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(
            ext.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
        )
    } else {
        false
    }
}

/// 特定のタグを持つ画像を検索
pub fn find_images_with_tag(dir: &Path, tag: &str) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if is_supported_format(&path) {
                let tags = load_tags(&path);
                if tags.iter().any(|t| t == tag) {
                    result.push(path);
                }
            }
        }
    }
    result.sort();
    result
}
