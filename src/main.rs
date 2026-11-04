mod config;
mod geom;

fn main() {
    let _settings = config::Settings::load(std::path::Path::new("settings.json"));
}
