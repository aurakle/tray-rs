fn main() {
    #[cfg(all(target_os = "linux", feature = "qt"))]
    {
        use cxx_qt_build::CxxQtBuilder;

        CxxQtBuilder::new()
            .qt_module("Widgets")
            .file("src/linux.rs")
            .cc_builder(|cc| {
                cc.include("include");
            })
            .build();
    }
}
