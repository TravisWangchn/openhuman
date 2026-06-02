use std::time::Duration;

fn main() {
    let client = reqwest::blocking::Client::builder()
        .use_native_tls()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("build client");

    println!("[verify] Client built (native-tls / schannel)");

    match client.get("https://api.deepseek.com/v1/models").send() {
        Ok(resp) => {
            let status = resp.status();
            println!("[verify] GET /v1/models -> {}", status);
            if status.is_success() || status.as_u16() == 401 {
                println!("[verify] PASS: TLS connection to api.deepseek.com works");
            } else {
                eprintln!("[verify] FAIL: unexpected status {}", status);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("[verify] FAIL: {e}");
            std::process::exit(1);
        }
    }

    let rustls_client = reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("build rustls client");

    match rustls_client
        .get("https://api.deepseek.com/v1/models")
        .send()
    {
        Ok(resp) => println!("[verify] rustls also works (status: {})", resp.status()),
        Err(e) => println!("[verify] rustls FAILS: {e} — native-tls is required"),
    }

    println!("[verify] Done.");
}
