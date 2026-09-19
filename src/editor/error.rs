use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub(crate) enum EditorError {
    #[error("could not create temporary editor file")]
    CreateTempFile {
        #[source]
        source: std::io::Error,
    },

    #[error("could not persist temporary editor file")]
    PersistTempFile {
        #[source]
        source: std::io::Error,
    },

    #[error("could not write temporary editor file {path}")]
    WriteTempFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read temporary editor file {path}")]
    ReadTempFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not launch editor `{program}`")]
    Launch {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("editor `{program}` failed: {message}")]
    Failed { program: String, message: String },

    #[error("could not parse editor command")]
    ParseCommand { command: String },
}
