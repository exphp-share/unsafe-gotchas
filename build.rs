fn main() {
    let mut files = vec![];
    files.push("README.md".into());
    files.extend(skeptic::markdown_files_of_directory("src/"));
    skeptic::generate_doc_tests(&files);
}
