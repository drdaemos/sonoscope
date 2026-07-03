use crate::error::CommandError;
use futures_util::StreamExt;
use serde::Serialize;
use specta::Type;
use specta_typescript::Number;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

const CLAP_REPO_ID: &str = "laion/larger_clap_music";
const CLAP_REPO_REVISION: &str = "main";
const ESSENTIA_MODEL_ID: &str = "essentia/fs_loop_ds-msd-musicnn";
const ESSENTIA_MUSICNN_URL: &str =
    "https://essentia.upf.edu/models/feature-extractors/musicnn/msd-musicnn-1.pb";
const ESSENTIA_LOOP_ROLE_URL: &str =
    "https://essentia.upf.edu/models/classification-heads/fs_loop_ds/fs_loop_ds-msd-musicnn-1.pb";
const REQUIRED_CLAP_FILES: &[&str] = &[
    "config.json",
    "merges.txt",
    "preprocessor_config.json",
    "pytorch_model.bin",
    "special_tokens_map.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "vocab.json",
];
const REQUIRED_ESSENTIA_FILES: &[(&str, &str)] = &[
    ("msd-musicnn-1.pb", ESSENTIA_MUSICNN_URL),
    ("fs_loop_ds-msd-musicnn-1.pb", ESSENTIA_LOOP_ROLE_URL),
];

#[derive(Debug, Clone, Serialize, Type)]
pub struct MlModelStatus {
    pub model_id: String,
    pub found: bool,
    pub path: String,
    pub missing_files: Vec<String>,
    #[specta(type = Number<u64>)]
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MlModelDownloadProgress {
    pub file_name: String,
    pub file_index: usize,
    pub file_count: usize,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

pub fn clap_model_dir() -> Result<PathBuf, CommandError> {
    let candidates = clap_model_dir_candidates();
    for candidate in &candidates {
        if clap_model_status_for_dir(candidate)?.found {
            return Ok(candidate.clone());
        }
    }
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| CommandError::Other("No model cache directory candidates".to_string()))
}

pub fn essentia_model_dir() -> Result<PathBuf, CommandError> {
    let candidates = essentia_model_dir_candidates();
    for candidate in &candidates {
        if essentia_model_status_for_dir(candidate)?.found {
            return Ok(candidate.clone());
        }
    }
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| CommandError::Other("No model cache directory candidates".to_string()))
}

fn clap_model_dir_candidates() -> Vec<PathBuf> {
    model_dir_candidates("SONOSCOPE_CLAP_MODEL_DIR", clap_model_dir_under)
}

fn essentia_model_dir_candidates() -> Vec<PathBuf> {
    model_dir_candidates("SONOSCOPE_ESSENTIA_MODEL_DIR", essentia_model_dir_under)
}

fn model_dir_candidates(env_name: &str, model_dir_under: fn(&Path) -> PathBuf) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os(env_name) {
        push_unique(&mut candidates, PathBuf::from(path));
    }
    if let Some(app_data) = std::env::var_os("APPDATA") {
        let app_data = PathBuf::from(app_data);
        push_unique(
            &mut candidates,
            model_dir_under(&app_data.join("com.sonoscope.app")),
        );
        push_unique(
            &mut candidates,
            model_dir_under(&app_data.join("Sonoscope")),
        );
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        push_unique(
            &mut candidates,
            model_dir_under(&local_app_data.join("com.sonoscope.app")),
        );
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        push_unique(
            &mut candidates,
            model_dir_under(
                &home
                    .join("Library")
                    .join("Application Support")
                    .join("com.sonoscope.app"),
            ),
        );
        push_unique(
            &mut candidates,
            model_dir_under(&home.join(".local").join("share").join("com.sonoscope.app")),
        );
    }
    candidates
}

fn clap_model_dir_under(root: &Path) -> PathBuf {
    root.join("models").join("laion").join("larger_clap_music")
}

fn essentia_model_dir_under(root: &Path) -> PathBuf {
    root.join("models")
        .join("essentia")
        .join("fs_loop_ds-msd-musicnn")
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

pub fn clap_model_status_for_dir(path: &Path) -> Result<MlModelStatus, CommandError> {
    let mut missing_files = Vec::new();
    let mut size_bytes = 0_u64;

    for file_name in REQUIRED_CLAP_FILES {
        let file_path = path.join(file_name);
        match std::fs::metadata(&file_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                size_bytes = size_bytes.saturating_add(metadata.len());
            }
            _ => missing_files.push((*file_name).to_string()),
        }
    }

    Ok(MlModelStatus {
        model_id: CLAP_REPO_ID.to_string(),
        found: missing_files.is_empty(),
        path: path.to_string_lossy().to_string(),
        missing_files,
        size_bytes,
    })
}

pub fn essentia_model_status_for_dir(path: &Path) -> Result<MlModelStatus, CommandError> {
    model_status_for_files(
        ESSENTIA_MODEL_ID,
        path,
        REQUIRED_ESSENTIA_FILES
            .iter()
            .map(|(file_name, _url)| *file_name),
    )
}

pub fn ml_model_status_for_dirs(
    clap_path: &Path,
    essentia_path: &Path,
) -> Result<MlModelStatus, CommandError> {
    let clap = clap_model_status_for_dir(clap_path)?;
    let essentia = essentia_model_status_for_dir(essentia_path)?;
    let mut missing_files = Vec::new();
    missing_files.extend(
        clap.missing_files
            .into_iter()
            .map(|file_name| format!("clap/{file_name}")),
    );
    missing_files.extend(
        essentia
            .missing_files
            .into_iter()
            .map(|file_name| format!("essentia/{file_name}")),
    );

    Ok(MlModelStatus {
        model_id: format!("{CLAP_REPO_ID} + {ESSENTIA_MODEL_ID}"),
        found: missing_files.is_empty(),
        path: format!(
            "CLAP: {}; Essentia: {}",
            clap_path.to_string_lossy(),
            essentia_path.to_string_lossy()
        ),
        missing_files,
        size_bytes: clap.size_bytes.saturating_add(essentia.size_bytes),
    })
}

fn model_status_for_files<'a>(
    model_id: &str,
    path: &Path,
    required_files: impl Iterator<Item = &'a str>,
) -> Result<MlModelStatus, CommandError> {
    let mut missing_files = Vec::new();
    let mut size_bytes = 0_u64;

    for file_name in required_files {
        let file_path = path.join(file_name);
        match std::fs::metadata(&file_path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                size_bytes = size_bytes.saturating_add(metadata.len());
            }
            _ => missing_files.push(file_name.to_string()),
        }
    }

    Ok(MlModelStatus {
        model_id: model_id.to_string(),
        found: missing_files.is_empty(),
        path: path.to_string_lossy().to_string(),
        missing_files,
        size_bytes,
    })
}

pub async fn download_clap_model_to_dir(
    app: &AppHandle,
    path: &Path,
) -> Result<MlModelStatus, CommandError> {
    tokio::fs::create_dir_all(path).await?;
    let current_status = clap_model_status_for_dir(path)?;
    if current_status.found {
        emit_complete(app, &current_status);
        return Ok(current_status);
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60 * 60))
        .build()
        .map_err(|e| CommandError::Other(format!("Failed to initialize downloader: {e}")))?;

    for (index, file_name) in REQUIRED_CLAP_FILES.iter().enumerate() {
        download_file(
            app,
            &client,
            file_name,
            &path.join(file_name),
            index + 1,
            REQUIRED_CLAP_FILES.len(),
        )
        .await?;
    }

    let status = clap_model_status_for_dir(path)?;
    emit_complete(app, &status);
    Ok(status)
}

pub async fn download_ml_models_to_dirs(
    app: &AppHandle,
    clap_path: &Path,
    essentia_path: &Path,
) -> Result<MlModelStatus, CommandError> {
    tokio::fs::create_dir_all(clap_path).await?;
    tokio::fs::create_dir_all(essentia_path).await?;
    let current_status = ml_model_status_for_dirs(clap_path, essentia_path)?;
    if current_status.found {
        emit_complete(app, &current_status);
        return Ok(current_status);
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60 * 60))
        .build()
        .map_err(|e| CommandError::Other(format!("Failed to initialize downloader: {e}")))?;

    let file_count = REQUIRED_CLAP_FILES.len() + REQUIRED_ESSENTIA_FILES.len();
    let mut file_index = 1;
    for file_name in REQUIRED_CLAP_FILES {
        let url = format!(
            "https://huggingface.co/{CLAP_REPO_ID}/resolve/{CLAP_REPO_REVISION}/{file_name}"
        );
        download_url_to_file(
            app,
            &client,
            &format!("clap/{file_name}"),
            &url,
            &clap_path.join(file_name),
            file_index,
            file_count,
        )
        .await?;
        file_index += 1;
    }
    for (file_name, url) in REQUIRED_ESSENTIA_FILES {
        download_url_to_file(
            app,
            &client,
            &format!("essentia/{file_name}"),
            url,
            &essentia_path.join(file_name),
            file_index,
            file_count,
        )
        .await?;
        file_index += 1;
    }

    let status = ml_model_status_for_dirs(clap_path, essentia_path)?;
    emit_complete(app, &status);
    Ok(status)
}

async fn download_file(
    app: &AppHandle,
    client: &reqwest::Client,
    file_name: &str,
    destination: &Path,
    file_index: usize,
    file_count: usize,
) -> Result<(), CommandError> {
    let url =
        format!("https://huggingface.co/{CLAP_REPO_ID}/resolve/{CLAP_REPO_REVISION}/{file_name}");
    download_url_to_file(
        app,
        client,
        file_name,
        &url,
        destination,
        file_index,
        file_count,
    )
    .await
}

async fn download_url_to_file(
    app: &AppHandle,
    client: &reqwest::Client,
    file_name: &str,
    url: &str,
    destination: &Path,
    file_index: usize,
    file_count: usize,
) -> Result<(), CommandError> {
    if let Ok(metadata) = tokio::fs::metadata(destination).await {
        if metadata.is_file() && metadata.len() > 0 {
            emit_progress(
                app,
                file_name,
                file_index,
                file_count,
                metadata.len(),
                Some(metadata.len()),
            );
            return Ok(());
        }
    }

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| CommandError::Other(format!("Failed to download {file_name}: {e}")))?
        .error_for_status()
        .map_err(|e| CommandError::Other(format!("Failed to download {file_name}: {e}")))?;
    let total_bytes = response.content_length();
    emit_progress(app, file_name, file_index, file_count, 0, total_bytes);

    let temporary_destination = destination.with_extension("part");
    let mut file = tokio::fs::File::create(&temporary_destination).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded_bytes = 0_u64;

    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| CommandError::Other(format!("Failed reading {file_name}: {e}")))?;
        downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);
        file.write_all(&chunk).await?;
        emit_progress(
            app,
            file_name,
            file_index,
            file_count,
            downloaded_bytes,
            total_bytes,
        );
        if total_bytes.is_some_and(|expected_bytes| downloaded_bytes >= expected_bytes) {
            break;
        }
    }
    file.flush().await?;
    drop(file);

    tokio::fs::rename(&temporary_destination, destination).await?;
    Ok(())
}

fn emit_progress(
    app: &AppHandle,
    file_name: &str,
    file_index: usize,
    file_count: usize,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    app.emit(
        "ml-model-download-progress",
        MlModelDownloadProgress {
            file_name: file_name.to_string(),
            file_index,
            file_count,
            downloaded_bytes,
            total_bytes,
        },
    )
    .ok();
}

fn emit_complete(app: &AppHandle, status: &MlModelStatus) {
    app.emit("ml-model-download-complete", status).ok();
}

#[cfg(test)]
mod tests {
    use super::{
        clap_model_dir_under, clap_model_status_for_dir, essentia_model_dir_under,
        essentia_model_status_for_dir, ml_model_status_for_dirs, push_unique,
    };
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn model_status_reports_missing_files() {
        let dir = TempDir::new().unwrap();

        let status = clap_model_status_for_dir(dir.path()).unwrap();

        assert!(!status.found);
        assert!(status
            .missing_files
            .contains(&"pytorch_model.bin".to_string()));
    }

    #[test]
    fn model_status_reports_ready_when_required_files_exist() {
        let dir = TempDir::new().unwrap();
        for file_name in [
            "config.json",
            "merges.txt",
            "preprocessor_config.json",
            "pytorch_model.bin",
            "special_tokens_map.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "vocab.json",
        ] {
            fs::write(dir.path().join(file_name), b"x").unwrap();
        }

        let status = clap_model_status_for_dir(dir.path()).unwrap();

        assert!(status.found);
        assert!(status.missing_files.is_empty());
        assert_eq!(status.size_bytes, 8);
    }

    #[test]
    fn model_dir_under_uses_laion_clap_subdirectory() {
        let root = PathBuf::from("app-data");

        let path = clap_model_dir_under(&root);

        assert_eq!(
            path,
            PathBuf::from("app-data")
                .join("models")
                .join("laion")
                .join("larger_clap_music")
        );
    }

    #[test]
    fn essentia_model_status_reports_required_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("msd-musicnn-1.pb"), b"x").unwrap();

        let status = essentia_model_status_for_dir(dir.path()).unwrap();

        assert!(!status.found);
        assert!(status
            .missing_files
            .contains(&"fs_loop_ds-msd-musicnn-1.pb".to_string()));
    }

    #[test]
    fn combined_model_status_reports_all_missing_files() {
        let clap_dir = TempDir::new().unwrap();
        let essentia_dir = TempDir::new().unwrap();

        let status = ml_model_status_for_dirs(clap_dir.path(), essentia_dir.path()).unwrap();

        assert!(!status.found);
        assert!(status
            .missing_files
            .contains(&"clap/pytorch_model.bin".to_string()));
        assert!(status
            .missing_files
            .contains(&"essentia/msd-musicnn-1.pb".to_string()));
    }

    #[test]
    fn essentia_model_dir_under_uses_loop_role_subdirectory() {
        let root = PathBuf::from("app-data");

        let path = essentia_model_dir_under(&root);

        assert_eq!(
            path,
            PathBuf::from("app-data")
                .join("models")
                .join("essentia")
                .join("fs_loop_ds-msd-musicnn")
        );
    }

    #[test]
    fn push_unique_ignores_duplicate_paths() {
        let mut paths = Vec::new();
        let path = PathBuf::from("same");

        push_unique(&mut paths, path.clone());
        push_unique(&mut paths, path);

        assert_eq!(paths.len(), 1);
    }
}
