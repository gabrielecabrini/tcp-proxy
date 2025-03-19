use config::{Config, File};
use directories::ProjectDirs;
use once_cell::sync::OnceCell;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

const DEFAULT_CONFIG: &str = include_str!("../config.yml");
static CONFIG: OnceCell<Arc<Settings>> = OnceCell::new();

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub debug: bool,
    pub forward: Vec<Server>,
}

#[derive(Debug, Deserialize)]
pub struct Server {
    pub name: String,
    #[serde(rename = "listen-address")]
    pub listen_address: String,
    #[serde(rename = "forward-address")]
    pub forward_address: String,
}

pub fn init_config() {
    let settings = load_config();
    CONFIG
        .set(Arc::new(settings))
        .expect("Config already initialized");
}

pub fn get_config() -> &'static Settings {
    CONFIG.get().expect("Config not initialized")
}

fn get_config_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("it", "gabrielecabrini", "tcp-proxy") {
        proj_dirs.config_dir().join("config.yml")
    } else {
        PathBuf::from("config.yml") // fallback
    }
}

fn ensure_config_file(config_path: &PathBuf) {
    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).expect("Couldn't create config directory");
        }
        fs::write(config_path, DEFAULT_CONFIG).expect("Couldn't write config file");
    }
}

pub fn load_config() -> Settings {
    let config_path = get_config_path();
    ensure_config_file(&config_path);

    let settings = Config::builder()
        .add_source(File::from(config_path))
        .build()
        .unwrap();

    match settings.try_deserialize() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Error while loading config: {}", e);
            std::process::exit(1);
        }
    }
}
