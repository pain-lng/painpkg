// Pain package manager (painpkg)
// Manages dependencies and packages for Pain projects

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod install;
mod manifest;
mod package;
mod registry;
mod resolver;

pub use manifest::Manifest;
pub use package::Package;
use registry::Registry;
use resolver::DependencyResolver;

#[derive(Parser)]
#[command(name = "painpkg")]
#[command(about = "Pain package manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Pain project
    Init {
        /// Project name (optional, defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Add a dependency to the project
    Add {
        /// Package name
        package: String,
        /// Version constraint (e.g., "1.0.0", "^1.0.0", "~1.0.0")
        #[arg(short, long)]
        version: Option<String>,
        /// Add as dev dependency
        #[arg(long)]
        dev: bool,
    },
    /// Remove a dependency
    Remove {
        /// Package name
        package: String,
    },
    /// Install all dependencies
    Install,
    /// Update dependencies
    Update {
        /// Update specific package (optional)
        package: Option<String>,
    },
    /// List installed packages
    List,
    /// Build the project
    Build {
        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Publish package to registry
    Publish {
        /// Registry URL (optional, defaults to local registry)
        #[arg(short, long)]
        registry: Option<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => {
            init_project(name)?;
        }
        Commands::Add {
            package,
            version,
            dev,
        } => {
            add_dependency(&package, version.as_deref(), dev)?;
        }
        Commands::Remove { package } => {
            remove_dependency(&package)?;
        }
        Commands::Install => {
            install_dependencies()?;
        }
        Commands::Update { package } => {
            update_dependencies(package.as_deref())?;
        }
        Commands::List => {
            list_packages()?;
        }
        Commands::Build { output } => {
            build_project(output)?;
        }
        Commands::Publish { registry } => {
            publish_package(registry.as_deref())?;
        }
    }

    Ok(())
}

fn init_project(name: Option<String>) -> anyhow::Result<()> {
    let current_dir = std::env::current_dir()?;
    let project_name = name.unwrap_or_else(|| {
        current_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-project")
            .to_string()
    });

    let manifest_path = current_dir.join("pain.toml");
    if manifest_path.exists() {
        return Err(anyhow::anyhow!(
            "pain.toml already exists in this directory"
        ));
    }

    let manifest = Manifest::new(&project_name);
    manifest.write_to_file(&manifest_path)?;

    // Create src directory
    let src_dir = current_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Create main.pain
    let main_file = src_dir.join("main.pain");
    if !main_file.exists() {
        std::fs::write(
            &main_file,
            r#"fn main():
    print("Hello, Pain!")
"#,
        )?;
    }

    println!("✓ Initialized Pain project: {}", project_name);
    println!("✓ Created pain.toml");
    println!("✓ Created src/main.pain");

    Ok(())
}

fn add_dependency(package: &str, version: Option<&str>, dev: bool) -> anyhow::Result<()> {
    let manifest_path = find_manifest()?;
    let mut manifest = Manifest::load_from_file(&manifest_path)?;

    let version_constraint = version.unwrap_or("*").to_string();

    if dev {
        manifest.add_dev_dependency(package, &version_constraint)?;
    } else {
        manifest.add_dependency(package, &version_constraint)?;
    }

    manifest.write_to_file(&manifest_path)?;
    println!("✓ Added dependency: {} {}", package, version_constraint);

    Ok(())
}

fn remove_dependency(package: &str) -> anyhow::Result<()> {
    let manifest_path = find_manifest()?;
    let mut manifest = Manifest::load_from_file(&manifest_path)?;

    manifest.remove_dependency(package)?;
    manifest.write_to_file(&manifest_path)?;
    println!("✓ Removed dependency: {}", package);

    Ok(())
}

fn install_dependencies() -> anyhow::Result<()> {
    let manifest_path = find_manifest()?;
    let manifest = Manifest::load_from_file(&manifest_path)?;

    println!("Resolving dependencies...");
    let registry = Registry::new()?;
    let mut resolver = DependencyResolver::new(registry);
    let resolved = resolver.resolve(&manifest)?;

    println!("Installing {} packages...", resolved.len());
    install::install_packages(&resolved)?;

    println!("✓ All dependencies installed");

    Ok(())
}

fn update_dependencies(package: Option<&str>) -> anyhow::Result<()> {
    let manifest_path = find_manifest()?;
    let manifest = Manifest::load_from_file(&manifest_path)?;

    let registry = Registry::new()?;
    let mut resolver = DependencyResolver::new(registry);

    if let Some(pkg) = package {
        // Update specific package
        println!("Updating {}...", pkg);
        // TODO: Implement specific package update
    } else {
        // Update all packages
        println!("Updating all dependencies...");
        let resolved = resolver.resolve(&manifest)?;
        install::install_packages(&resolved)?;
    }

    println!("✓ Dependencies updated");

    Ok(())
}

fn list_packages() -> anyhow::Result<()> {
    let manifest_path = find_manifest()?;
    let manifest = Manifest::load_from_file(&manifest_path)?;

    println!("Project: {}", manifest.name);
    println!("\nDependencies:");
    for (name, version) in &manifest.dependencies {
        println!("  {} {}", name, version);
    }

    if !manifest.dev_dependencies.is_empty() {
        println!("\nDev Dependencies:");
        for (name, version) in &manifest.dev_dependencies {
            println!("  {} {}", name, version);
        }
    }

    Ok(())
}

fn build_project(_output: Option<PathBuf>) -> anyhow::Result<()> {
    let manifest_path = find_manifest()?;
    let manifest = Manifest::load_from_file(&manifest_path)?;

    println!("Building project: {}...", manifest.name);

    // TODO: Integrate with pain-compiler
    // For now, just check that dependencies are installed
    let registry = Registry::new()?;
    let mut resolver = DependencyResolver::new(registry);
    let _resolved = resolver.resolve(&manifest)?;

    println!("✓ Build complete");

    Ok(())
}

fn publish_package(_registry_url: Option<&str>) -> anyhow::Result<()> {
    let manifest_path = find_manifest()?;
    let manifest = Manifest::load_from_file(&manifest_path)?;

    println!(
        "Publishing package: {} v{}...",
        manifest.name, manifest.version
    );

    // Validate the manifest
    manifest.validate()?;

    // Load package from current directory
    let current_dir = std::env::current_dir()?;
    let package = Package::load_from_path(&current_dir)?;

    // Publish to local registry
    let registry = Registry::new()?;
    registry.publish_package(&package)?;

    println!("✓ Package published to local registry");

    Ok(())
}

fn find_manifest() -> anyhow::Result<PathBuf> {
    let current_dir = std::env::current_dir()?;
    let manifest_path = current_dir.join("pain.toml");

    if !manifest_path.exists() {
        return Err(anyhow::anyhow!(
            "pain.toml not found. Run 'painpkg init' to create a new project."
        ));
    }

    Ok(manifest_path)
}
