use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{Args, Subcommand};
use ostool::build::CargoQemuOverrideArgs;

use crate::{
    command_flow::{self, SnapshotPersistence},
    context::{
        AppContext, DEFAULT_STARRY_ARCH, QemuRunConfig, ResolvedStarryRequest, StarryCliArgs,
        starry_target_for_arch_checked,
    },
    test_qemu,
};

pub mod board;
pub mod build;
pub mod config;
pub mod rootfs;
pub mod test_suit;

/// StarryOS subcommands
#[derive(Subcommand)]
pub enum Command {
    /// Build StarryOS application
    Build(ArgsBuild),
    /// Build and run StarryOS application
    Qemu(ArgsQemu),
    /// Generate a default StarryOS board config
    Defconfig(ArgsDefconfig),
    /// StarryOS board config helpers
    Config(ArgsConfig),
    /// Run StarryOS test suites
    Test(ArgsTest),
    /// Download rootfs image into workspace target directory
    Rootfs(ArgsRootfs),
    /// Build and run StarryOS application with U-Boot
    Uboot(ArgsUboot),
}

#[derive(Args, Clone)]
pub struct ArgsBuild {
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub arch: Option<String>,
    #[arg(short, long)]
    pub target: Option<String>,
    #[arg(long = "plat_dyn", alias = "plat-dyn")]
    pub plat_dyn: Option<bool>,

    #[arg(long)]
    pub debug: bool,
}

#[derive(Args)]
pub struct ArgsQemu {
    #[command(flatten)]
    pub build: ArgsBuild,

    #[arg(long)]
    pub qemu_config: Option<PathBuf>,
}

#[derive(Args)]
pub struct ArgsUboot {
    #[command(flatten)]
    pub build: ArgsBuild,

    #[arg(long)]
    pub uboot_config: Option<PathBuf>,
}

#[derive(Args)]
pub struct ArgsRootfs {
    #[arg(long)]
    pub arch: Option<String>,
}

#[derive(Args)]
pub struct ArgsDefconfig {
    pub board: String,
}

#[derive(Args)]
pub struct ArgsConfig {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Args)]
pub struct ArgsTest {
    #[command(subcommand)]
    pub command: TestCommand,
}

#[derive(Subcommand)]
pub enum TestCommand {
    /// Run StarryOS QEMU test suite
    Qemu(ArgsTestQemu),
    /// Reserved StarryOS U-Boot test suite entrypoint
    Uboot(ArgsTestUboot),
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// List available board names
    Ls,
}

#[derive(Args, Debug, Clone)]
pub struct ArgsTestQemu {
    #[arg(short = 't', long, alias = "arch", value_name = "ARCH")]
    pub target: String,
    #[arg(short = 'c', long, value_name = "CASE")]
    pub test_case: Option<String>,
    #[arg(long, value_name = "CMD_OR_FILE")]
    pub shell_init_cmd: Option<String>,
    #[arg(
        long,
        value_name = "SECONDS",
        help = "Test timeout in seconds (0 to disable timeout)"
    )]
    pub timeout: Option<u64>,
}

#[derive(Args, Debug, Clone, Default)]
pub struct ArgsTestUboot;

pub struct Starry {
    app: AppContext,
}

impl From<&ArgsBuild> for StarryCliArgs {
    fn from(args: &ArgsBuild) -> Self {
        Self {
            config: args.config.clone(),
            arch: args.arch.clone(),
            target: args.target.clone(),
            plat_dyn: args.plat_dyn,
            debug: args.debug,
        }
    }
}

impl Starry {
    pub fn new() -> anyhow::Result<Self> {
        let app = AppContext::new()?;
        Ok(Self { app })
    }

    pub async fn execute(&mut self, command: Command) -> anyhow::Result<()> {
        match command {
            Command::Build(args) => self.build(args).await,
            Command::Qemu(args) => self.qemu(args).await,
            Command::Defconfig(args) => self.defconfig(args),
            Command::Config(args) => self.config(args),
            Command::Rootfs(args) => self.rootfs(args).await,
            Command::Uboot(args) => self.uboot(args).await,
            Command::Test(args) => self.test(args).await,
        }
    }

    async fn build(&mut self, args: ArgsBuild) -> anyhow::Result<()> {
        let request =
            self.prepare_request((&args).into(), None, None, SnapshotPersistence::Store)?;
        self.run_build_request(request).await
    }

    async fn qemu(&mut self, args: ArgsQemu) -> anyhow::Result<()> {
        let request = self.prepare_request(
            (&args.build).into(),
            args.qemu_config,
            None,
            SnapshotPersistence::Store,
        )?;
        self.run_qemu_request(request).await
    }

    async fn rootfs(&mut self, args: ArgsRootfs) -> anyhow::Result<()> {
        let arch = args.arch.unwrap_or_else(|| DEFAULT_STARRY_ARCH.to_string());
        let target = starry_target_for_arch_checked(&arch)?.to_string();
        let disk_img =
            rootfs::ensure_rootfs_in_target_dir(self.app.workspace_root(), &arch, &target).await?;
        println!("rootfs ready at {}", disk_img.display());
        Ok(())
    }

    fn defconfig(&mut self, args: ArgsDefconfig) -> anyhow::Result<()> {
        let path = config::write_defconfig(self.app.workspace_root(), &args.board)?;
        println!("Generated {} for board {}", path.display(), args.board);
        Ok(())
    }

    fn config(&mut self, args: ArgsConfig) -> anyhow::Result<()> {
        match args.command {
            ConfigCommand::Ls => {
                for board in config::available_board_names(self.app.workspace_root())? {
                    println!("{board}");
                }
            }
        }
        Ok(())
    }

    async fn uboot(&mut self, args: ArgsUboot) -> anyhow::Result<()> {
        let request = self.prepare_request(
            (&args.build).into(),
            None,
            args.uboot_config,
            SnapshotPersistence::Store,
        )?;
        self.run_uboot_request(request).await
    }

    fn resolve_shell_init_cmd(input: Option<String>) -> anyhow::Result<Option<String>> {
        match input {
            None => Ok(None),
            Some(value) => {
                let path = Path::new(&value);
                if path.exists() {
                    let content = fs::read_to_string(path).with_context(|| {
                        format!("failed to read shell init cmd file: {}", path.display())
                    })?;
                    // Join multiple commands with &&
                    let content = content
                        .lines()
                        .map(|line| line.trim())
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join(" && ");
                    Ok(Some(content))
                } else {
                    Ok(Some(value))
                }
            }
        }
    }

    async fn test(&mut self, args: ArgsTest) -> anyhow::Result<()> {
        match args.command {
            TestCommand::Qemu(args) => self.test_qemu(args).await,
            TestCommand::Uboot(args) => self.test_uboot(args).await,
        }
    }

    async fn test_qemu(&mut self, args: ArgsTestQemu) -> anyhow::Result<()> {
        let (arch, target) = test_suit::parse_test_target(self.app.workspace_root(), &args.target)?;
        let cases = test_suit::discover_qemu_cases(
            self.app.workspace_root(),
            &arch,
            args.test_case.as_deref(),
        )?;
        let package = crate::context::STARRY_PACKAGE;
        let shell_init_cmd = Self::resolve_shell_init_cmd(args.shell_init_cmd)?;

        println!(
            "running starry qemu tests for package {} on arch: {} (target: {})",
            package, arch, target
        );

        config::write_default_qemu_defconfig_for_target(self.app.workspace_root(), &target)?;
        let request = self.prepare_request(
            Self::test_build_args(&arch),
            None,
            None,
            SnapshotPersistence::Discard,
        )?;

        let total = cases.len();
        let mut failed = Vec::new();
        for (index, case) in cases.iter().enumerate() {
            println!("[{}/{}] starry qemu {}", index + 1, total, case.name);
            let qemu_config = test_suit::prepare_test_qemu_config(
                self.app.workspace_root(),
                &request,
                &case.qemu_config_path,
                args.timeout,
            )
            .await?;

            match self
                .run_test_qemu_request(request.clone(), qemu_config, shell_init_cmd.clone())
                .await
                .with_context(|| format!("starry qemu test failed for case `{}`", case.name))
            {
                Ok(()) => println!("ok: {}", case.name),
                Err(err) => {
                    eprintln!("failed: {}: {:#}", case.name, err);
                    failed.push(case.name.clone());
                }
            }
        }

        test_suit::finalize_qemu_case_run(&failed)
    }

    async fn test_uboot(&mut self, _args: ArgsTestUboot) -> anyhow::Result<()> {
        test_qemu::unsupported_uboot_test_command("starry")
    }

    fn prepare_request(
        &self,
        args: StarryCliArgs,
        qemu_config: Option<PathBuf>,
        uboot_config: Option<PathBuf>,
        persistence: SnapshotPersistence,
    ) -> anyhow::Result<ResolvedStarryRequest> {
        command_flow::resolve_request(
            persistence,
            || {
                self.app
                    .prepare_starry_request(args, qemu_config, uboot_config)
            },
            |snapshot| self.app.store_starry_snapshot(snapshot),
        )
    }

    fn test_build_args(arch: &str) -> StarryCliArgs {
        StarryCliArgs {
            config: None,
            arch: Some(arch.to_string()),
            target: None,
            plat_dyn: None,
            debug: false,
        }
    }

    fn qemu_run_config(
        qemu_config: Option<PathBuf>,
        qemu_args: Vec<String>,
    ) -> anyhow::Result<QemuRunConfig> {
        Ok(QemuRunConfig {
            qemu_config,
            default_args: CargoQemuOverrideArgs {
                args: Some(qemu_args),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn run_qemu_request(&mut self, request: ResolvedStarryRequest) -> anyhow::Result<()> {
        let qemu_args = rootfs::default_qemu_args(self.app.workspace_root(), &request).await?;
        self.run_qemu_request_with_args(request, qemu_args).await
    }

    async fn run_qemu_request_with_args(
        &mut self,
        request: ResolvedStarryRequest,
        qemu_args: Vec<String>,
    ) -> anyhow::Result<()> {
        command_flow::run_qemu(
            &mut self.app,
            request,
            build::load_cargo_config,
            move |request| Self::qemu_run_config(request.qemu_config.clone(), qemu_args),
        )
        .await
    }

    async fn run_test_qemu_request(
        &mut self,
        request: ResolvedStarryRequest,
        qemu_config: PathBuf,
        shell_init_cmd_override: Option<String>,
    ) -> anyhow::Result<()> {
        let cargo = build::load_cargo_config(&request)?;

        // Use override_args if shell_init_cmd is provided
        let override_args = if let Some(cmd) = shell_init_cmd_override {
            CargoQemuOverrideArgs {
                shell_init_cmd: Some(cmd),
                ..Default::default()
            }
        } else {
            CargoQemuOverrideArgs::default()
        };

        self.app
            .qemu(
                cargo,
                request.build_info_path,
                QemuRunConfig {
                    qemu_config: Some(qemu_config),
                    override_args,
                    ..Default::default()
                },
            )
            .await
    }

    async fn run_build_request(&mut self, request: ResolvedStarryRequest) -> anyhow::Result<()> {
        command_flow::run_build(&mut self.app, request, build::load_cargo_config).await
    }

    async fn run_uboot_request(&mut self, request: ResolvedStarryRequest) -> anyhow::Result<()> {
        command_flow::run_uboot(&mut self.app, request, build::load_cargo_config).await
    }
}

impl Default for Starry {
    fn default() -> Self {
        Self::new().expect("failed to initialize StarryOS")
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn command_parses_test_qemu() {
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: Command,
        }

        let cli = Cli::try_parse_from(["starry", "test", "qemu", "--target", "x86_64"]).unwrap();

        match cli.command {
            Command::Test(args) => match args.command {
                TestCommand::Qemu(args) => assert_eq!(args.target, "x86_64"),
                _ => panic!("expected qemu test command"),
            },
            _ => panic!("expected test command"),
        }
    }

    #[test]
    fn command_parses_defconfig() {
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: Command,
        }

        let cli = Cli::try_parse_from(["starry", "defconfig", "qemu-aarch64"]).unwrap();

        match cli.command {
            Command::Defconfig(args) => assert_eq!(args.board, "qemu-aarch64"),
            _ => panic!("expected defconfig command"),
        }
    }

    #[test]
    fn command_parses_config_ls() {
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: Command,
        }

        let cli = Cli::try_parse_from(["starry", "config", "ls"]).unwrap();

        match cli.command {
            Command::Config(args) => match args.command {
                ConfigCommand::Ls => {}
            },
            _ => panic!("expected config ls command"),
        }
    }

    #[test]
    fn command_parses_test_uboot() {
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: Command,
        }

        let cli = Cli::try_parse_from(["starry", "test", "uboot"]).unwrap();

        match cli.command {
            Command::Test(args) => match args.command {
                TestCommand::Uboot(_) => {}
                _ => panic!("expected uboot test command"),
            },
            _ => panic!("expected test command"),
        }
    }

    #[test]
    fn command_parses_test_qemu_with_shell_init_cmd() {
        #[derive(Parser)]
        struct Cli {
            #[command(subcommand)]
            command: Command,
        }

        let cli = Cli::try_parse_from([
            "starry",
            "test",
            "qemu",
            "-t",
            "x86_64",
            "-c",
            "smoke",
            "--shell-init-cmd",
            "echo 'test'",
            "--timeout",
            "10",
        ])
        .unwrap();

        match cli.command {
            Command::Test(args) => match args.command {
                TestCommand::Qemu(args) => {
                    assert_eq!(args.target, "x86_64");
                    assert_eq!(args.test_case, Some("smoke".to_string()));
                    assert_eq!(args.shell_init_cmd, Some("echo 'test'".to_string()));
                    assert_eq!(args.timeout, Some(10));
                }
                _ => panic!("expected qemu test command"),
            },
            _ => panic!("expected test command"),
        }
    }

    #[test]
    fn resolve_shell_init_cmd_returns_none_for_none_input() {
        let result = Starry::resolve_shell_init_cmd(None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_shell_init_cmd_returns_value_for_nonexistent_path() {
        let result =
            Starry::resolve_shell_init_cmd(Some("echo 'direct command'".to_string())).unwrap();
        assert_eq!(result, Some("echo 'direct command'".to_string()));
    }

    #[test]
    fn resolve_shell_init_cmd_reads_file_content() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test-cmd.txt");
        fs::write(&file, "echo 'from file'\nls -la\n").unwrap();

        let result = Starry::resolve_shell_init_cmd(Some(file.display().to_string())).unwrap();
        assert_eq!(result, Some("echo 'from file' && ls -la".to_string()));
    }
}
