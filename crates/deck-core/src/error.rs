use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum DeckError {
    #[error("网络客户端初始化失败")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("请先解锁本地凭据库")]
    VaultLocked,
    #[error("口令不正确，或凭据库已损坏")]
    VaultAuthentication,
    #[error("凭据加密失败")]
    Encryption,
    #[error("应用正在退出，请稍后重启")]
    ShuttingDown,
    #[error("后台工作未在退出期限内完成；下次启动将核对中断记录")]
    ShutdownTimeout,
    #[error("后台工作线程异常结束")]
    Worker,
    #[error("数据库操作失败，请检查本地存储状态")]
    Storage(#[from] rusqlite::Error),
    #[error("无法访问应用数据目录")]
    Io(#[from] std::io::Error),
    #[error("数据格式损坏")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDto {
    pub code: &'static str,
    pub message: String,
    pub action: &'static str,
}

impl From<DeckError> for ErrorDto {
    fn from(error: DeckError) -> Self {
        let (code, action) = match &error {
            DeckError::Http(_) => ("network_setup", "检查系统网络环境"),
            DeckError::Invalid(_) => ("invalid_input", "检查输入后重试"),
            DeckError::VaultLocked => ("vault_locked", "解锁凭据库"),
            DeckError::VaultAuthentication => ("vault_authentication", "核对口令并保留原始数据库"),
            DeckError::Encryption => ("encryption_failed", "保留原数据并重试"),
            DeckError::ShuttingDown => ("shutting_down", "等待退出完成"),
            DeckError::ShutdownTimeout => ("shutdown_timeout", "重启后检查任务记录"),
            DeckError::Worker => ("worker_failed", "重启应用后检查任务记录"),
            DeckError::Storage(_) | DeckError::Io(_) | DeckError::Json(_) => {
                ("storage_failed", "检查磁盘空间和数据目录权限")
            }
        };
        Self {
            code,
            message: error.to_string(),
            action,
        }
    }
}

pub type Result<T> = std::result::Result<T, DeckError>;
