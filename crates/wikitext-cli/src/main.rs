use clap::Parser;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use wikitext::preprocess::MemoryTemplates;
use wikitext::{compile, Options, OutputMode};
#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
enum InputFormat {
    Html,
    Markdown,
    Wikitext,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
enum OutputFormat {
    Wikitext,
    Markdown,
    Html,
}

#[derive(Parser, Debug)]
#[command(name = "wikitext", version, about, long_about = None)]
struct Arguments {
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
    /// Read the input as another format. Defaults to wikitext.
    #[arg(long, value_name = "FORMAT")]
    from: Option<InputFormat>,
    /// Write another format instead of HTML. Defaults to HTML.
    #[arg(long, value_name = "FORMAT")]
    to: Option<OutputFormat>,
    #[arg(short = 'T', long)]
    title: Option<String>,
    #[arg(long, conflicts_with = "standalone")]
    wrap: bool,
    #[arg(long, conflicts_with = "wrap")]
    standalone: bool,
    #[arg(long)]
    fragment_div: bool,
    #[arg(short = 't', long = "templates", value_name = "DIR")]
    templates: Option<PathBuf>,
    #[arg(short = 'm', long = "modules", value_name = "DIR")]
    modules: Option<PathBuf>,
    #[arg(long)]
    diagnostics: bool,
    #[arg(long)]
    compact: bool,
    #[arg(long, default_value = "en")]
    language: String,
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    if arguments.from.is_some() || arguments.to.is_some() {
        return convert(arguments);
    }
    let mut options = Options {
        title: arguments.title.clone(),
        language: arguments.language.clone(),
        output_mode: if arguments.standalone {
            OutputMode::Document
        } else if arguments.wrap || arguments.fragment_div {
            OutputMode::ParserOutput
        } else {
            OutputMode::Fragment
        },
        pretty: !arguments.compact,
        ..Options::default()
    };

    if let Some(directory) = &arguments.templates {
        match load_templates(directory) {
            Ok(templates) => options.resolver = Some(Arc::new(templates)),
            Err(error) => {
                eprintln!("wikitext: cannot read templates from {directory:?}: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    if let Some(directory) = &arguments.modules {
        match load_modules(directory, arguments.title.as_deref()) {
            Ok(modules) => options.module_provider = Some(Rc::new(modules)),
            Err(error) => {
                eprintln!("wikitext: cannot read modules from {directory:?}: {error}");
                return ExitCode::FAILURE;
            }
        }
    }

    let mut sources: Vec<(String, String)> = Vec::new();
    if arguments.files.is_empty() {
        let mut buffer = String::new();
        if let Err(error) = std::io::stdin().read_to_string(&mut buffer) {
            eprintln!("wikitext: cannot read standard input: {error}");
            return ExitCode::FAILURE;
        }
        sources.push(("<stdin>".to_string(), buffer));
    } else {
        for path in &arguments.files {
            match std::fs::read_to_string(path) {
                Ok(text) => sources.push((path.display().to_string(), text)),
                Err(error) => {
                    eprintln!("wikitext: cannot read {}: {error}", path.display());
                    return ExitCode::FAILURE;
                }
            }
        }
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for (name, source) in sources {
        let html = compile(&source, &options);
        if let Err(error) = out.write_all(html.as_bytes()) {
            eprintln!("wikitext: cannot write output: {error}");
            return ExitCode::FAILURE;
        }
        if arguments.diagnostics {
            let document = wikitext::parse(&source, &options);
            for diagnostic in &document.diagnostics {
                eprintln!("{name}: {diagnostic}");
            }
        }
    }
    ExitCode::SUCCESS
}

fn convert(arguments: Arguments) -> ExitCode {
    let input = match read_input(&arguments) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("wikitext: {error}");
            return ExitCode::FAILURE;
        }
    };
    let input_format = arguments.from.unwrap_or(InputFormat::Wikitext);
    let output_format = arguments.to.unwrap_or(OutputFormat::Wikitext);
    let options = match input_format {
        InputFormat::Html => wikitext_convert::Options::html(),
        InputFormat::Markdown => wikitext_convert::Options::markdown(),
        InputFormat::Wikitext => wikitext_convert::Options::wikitext(),
    };
    let output = match output_format {
        OutputFormat::Wikitext => wikitext_convert::Output::Wikitext,
        OutputFormat::Markdown => wikitext_convert::Output::Markdown,
        OutputFormat::Html => wikitext_convert::Output::Html,
    };
    if print(wikitext_convert::convert(&input, &options, output)).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn read_input(arguments: &Arguments) -> Result<String, String> {
    if arguments.files.is_empty() {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|error| format!("cannot read standard input: {error}"))?;
        return Ok(buffer);
    }
    let mut all = String::new();
    for path in &arguments.files {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {path:?}: {error}"))?;
        all.push_str(&text);
        if !all.ends_with('\n') {
            all.push('\n');
        }
    }
    Ok(all)
}

fn print(text: String) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(text.as_bytes())?;
    if !text.ends_with('\n') {
        stdout.write_all(b"\n")?;
    }
    stdout.flush()
}

fn load_modules(
    directory: &PathBuf,
    title: Option<&str>,
) -> std::io::Result<wikitext::lua_modules::LuaModules> {
    let modules = wikitext::lua_modules::LuaModules::new();
    if let Some(title) = title {
        modules.set_title(title);
    }
    let mut stack = vec![directory.clone()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("lua") {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let relative = path
                .strip_prefix(directory)
                .unwrap_or(&path)
                .with_extension("");
            let name = relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            modules.insert(&name, source);
        }
    }
    Ok(modules)
}

fn load_templates(directory: &PathBuf) -> std::io::Result<MemoryTemplates> {
    let templates = MemoryTemplates::new();
    let mut stack = vec![directory.clone()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("");
            if !matches!(extension, "wikitext" | "wiki" | "txt" | "mediawiki") {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&path) else {
                continue;
            };
            let relative = path
                .strip_prefix(directory)
                .unwrap_or(&path)
                .with_extension("");
            let name = relative.to_string_lossy().replace('\\', "/");
            templates.insert(&name, body);
        }
    }
    Ok(templates)
}
