mod deploy;
pub use deploy::*;

mod init;
pub use init::*;

mod files;
pub use files::*;

pub trait SubcommandDelegate {
    fn run(self) -> Result<(), Box<dyn std::error::Error>>;
}
