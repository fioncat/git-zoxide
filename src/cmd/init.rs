use anyhow::Result;

use crate::cmd::Init;
use crate::cmd::Run;

const DEFAULT_CMD: &str = "gz";
const DEFAULT_HOME: &str = "zz";
const DEFAULT_JUMP: &str = "zj";

impl Run for Init {
    fn run(&self) -> Result<()> {
        let cmp_bytes = include_bytes!("../../scripts/_git-zoxide.zsh");
        println!("{}", String::from_utf8_lossy(cmp_bytes));

        let init_bytes = include_bytes!("../../scripts/init.zsh");
        let init = String::from_utf8_lossy(init_bytes);

        let cmd = if let Some(s) = &self.cmd {
            s.as_str()
        } else {
            DEFAULT_CMD
        };

        let home = if let Some(s) = &self.home_cmd {
            s.as_str()
        } else {
            DEFAULT_HOME
        };

        let jump = if let Some(s) = &self.jump_cmd {
            s.as_str()
        } else {
            DEFAULT_JUMP
        };

        let init = init
            .replace("{{CMD}}", cmd)
            .replace("{{HOME_CMD}}", home)
            .replace("{{JUMP_CMD}}", jump);
        println!("{}", init);

        Ok(())
    }
}
