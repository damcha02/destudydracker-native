fn main() {
  println!("cargo:rerun-if-env-changed=STUDY_TRACKER_OFFICIAL_RELEASE");
  if std::env::var("STUDY_TRACKER_OFFICIAL_RELEASE").as_deref() == Ok("1") {
    println!("cargo:rustc-env=STUDY_TRACKER_OFFICIAL_RELEASE=1");
  }

  tauri_build::build()
}
