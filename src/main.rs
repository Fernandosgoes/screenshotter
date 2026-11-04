mod config;
mod geom;
mod model;
mod pixelate;

fn main() {
    let _settings = config::Settings::load(std::path::Path::new("settings.json"));
}
