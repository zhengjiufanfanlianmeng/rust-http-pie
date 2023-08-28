use anyhow::{anyhow, Result};
use clap::Parser;
use colored::*;
use mime::Mime;
use reqwest::{header, Client, Response, Url};
use std::{collections::HashMap, str::FromStr};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, LinesWithEndings},
};

/// A naive httpie implementation with Rust, can you imagine how easy it is?
#[derive(Parser, Debug)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
}

// 子命令分别对应不同的 HTTP 方法，目前只支持 get / post
#[derive(Parser, Debug)]
enum SubCommand {
    Get(Get),
    Post(Post),
    // 我们暂且不支持其它 HTTP 方法
}

// get 子命令
#[derive(Parser, Debug)]
struct Get {
    /// HTTP 请求的 URL
    #[arg(value_parser = parse_url)]
    url: String,
}

#[derive(Parser, Debug)]
struct Post {
    /// HTTP 请求的 URL
    #[arg(value_parser = parse_url)]
    url: String,
    /// HTTP 请求的 body
    #[arg(value_parser = parse_kv_pair)]
    body: Vec<KvPair>,
}

#[derive(Debug, Clone)]
struct KvPair {
    k: String,
    v: String,
}

impl FromStr for KvPair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut split = s.split("=");
        let err = || anyhow!("Failed to parse {} as k=v format", s);
        Ok(Self {
            k: (split.next().ok_or_else(err)?).to_string(),
            v: (split.next().ok_or_else(err)?).to_string(),
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts: Opts = Opts::parse();

    let mut headers = header::HeaderMap::new();
    headers.insert("X-POWERED-BY", "Rust".parse()?);
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("rust-httpie"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let res = match opts.subcmd {
        SubCommand::Get(ref args) => get(client, args).await?,
        SubCommand::Post(ref args) => post(client, args).await?,
    };

    Ok(res)
}

async fn get(client: Client, args: &Get) -> Result<()> {
    let resp = client.get(&args.url).send().await?;
    Ok(print_response(resp).await?)
}

async fn post(client: Client, args: &Post) -> Result<()> {
    let mut body = HashMap::new();
    for kv in args.body.iter() {
        body.insert(&kv.k, &kv.v);
    }
    let resp = client.post(&args.url).json(&body).send().await?;
    Ok(print_response(resp).await?)
}

fn parse_url(s: &str) -> Result<String> {
    let _url: Url = s.parse()?;
    Ok(s.into())
}

fn parse_kv_pair(s: &str) -> Result<KvPair> {
    Ok(s.parse()?)
}

fn print_blue_status(resp: &Response) {
    let status = format!("{:?} {}", resp.version(), resp.status()).blue();
    println!("{}\n", status);
}

fn print_green_headers(resp: &Response) {
    for (name, value) in resp.headers() {
        println!(
            "{}: {}",
            name.to_string().green(),
            value.to_str().unwrap().bright_yellow()
        );
    }
    println!();
}

fn print_cyan_body(m: Option<Mime>, body: &String) {
    match m {
        Some(v) if v == mime::APPLICATION_JSON => print_syntect(body, "json"),
        Some(v) if v == mime::TEXT_HTML || v == mime::TEXT_HTML_UTF_8 => {
            print_syntect(body, "html")
        }
        // Some(v) if v == mime::TEXT_HTML_UTF_8 || v == mime::TEXT_HTML => {
        //     print_syntect(body, "html")
        // }
        _ => println!("{}", body.cyan()),
    }
}

async fn print_response(resp: Response) -> Result<()> {
    print_blue_status(&resp);
    print_green_headers(&resp);

    let mime = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().parse().unwrap());
    let body = resp.text().await?;
    print_cyan_body(mime, &body);

    Ok(())
}

fn print_syntect(s: &str, ext: &str) {
    // 将字符串按照指定语法进行高亮并打印的功能。
    // Load these once at the start of your program
    let ps = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = ps.find_syntax_by_extension(ext).unwrap();
    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
    for line in LinesWithEndings::from(s) {
        let ranges_result: Result<Vec<(Style, &str)>, _> = h.highlight_line(line, &ps);
        let ranges = ranges_result.unwrap(); // 或者使用 expect() 方法处理错误
        let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
        print!("{}", escaped);
    }
}
