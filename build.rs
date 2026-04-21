fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("RustDirStat.ico");
        res.compile().unwrap();
    }
}
