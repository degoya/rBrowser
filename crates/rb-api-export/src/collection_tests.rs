use super::*;
use crate::fixtures::create_user;
use rb_api_model::{Body, Part};

fn items() -> Vec<Item> {
    let mut upload = ApiRequest::new("PUT", "{{baseUrl}}/logo");
    upload.name = "Upload".into();
    upload.body = Body::Multipart {
        parts: vec![Part {
            name: "logo".into(),
            value: String::new(),
            file: Some("./img/logo.png".into()),
            content_type: None,
        }],
    };
    vec![
        Item {
            id: "shop/users/create.http".into(),
            request: create_user(),
        },
        Item {
            id: "shop/media/upload.http".into(),
            request: upload,
        },
    ]
}

#[test]
fn rebase_puts_a_path_relative_to_the_exported_folder() {
    assert_eq!(rebase("media", "./img/logo.png"), "./media/img/logo.png");
    assert_eq!(rebase("media", "../shared/a.bin"), "./shared/a.bin");
    assert_eq!(rebase("", "../../x.bin"), "../../x.bin");
    assert_eq!(rebase("a/b", "..\\..\\..\\x.bin"), "../x.bin");
    assert_eq!(rebase("a", "C:/data/x.bin"), "C:/data/x.bin");
    assert_eq!(rebase("a", "/abs/x.bin"), "/abs/x.bin");
}

#[test]
fn a_http_file_holds_every_request_its_files_rebased() {
    let out = export("Shop", "shop", &items(), Format::Http, Shell::Posix);
    assert!(out.text.starts_with("### Create user\n"), "{}", out.text);
    assert!(out.text.contains("\n### Upload\n"));
    assert!(
        out.text.contains("< ./media/img/logo.png\n"),
        "{}",
        out.text
    );
    assert!(out.env.is_empty());
}

#[test]
fn a_postman_collection_of_the_folder() {
    let out = export("Shop", "shop", &items(), Format::Postman, Shell::Posix);
    let value: serde_json::Value = serde_json::from_str(&out.text).expect("JSON");
    assert_eq!(value["info"]["name"], "Shop");
    assert_eq!(value["item"][0]["name"], "users");
    assert_eq!(value["item"][1]["name"], "media");
    assert_eq!(
        value["item"][1]["item"][0]["request"]["body"]["formdata"][0]["src"],
        "./media/img/logo.png"
    );
}

#[test]
fn a_curl_script_reads_every_variable_from_the_environment() {
    let out = export("Shop", "shop", &items(), Format::Curl, Shell::Posix);
    assert_eq!(
        out.text,
        "#!/bin/sh
# Shop: 2 requests from rBrowser's API client.
# Set first: token, baseUrl
# File paths are relative to .rbrowser/api/shop.

# Create user
curl --location --request POST https://api.example.com/users \\
  --header 'Accept: application/json' \\
  --header 'Authorization: Bearer '\"${token}\" \\
  --header 'Content-Type: application/json' \\
  --data-raw '{\"name\": \"Ada\"}'

# Upload
curl --location --request PUT \"${baseUrl}\"/logo \\
  --form logo=@./media/img/logo.png
"
    );
    assert_eq!(out.env, ["token", "baseUrl"]);
}

#[test]
fn a_cmd_script_starts_quietly_and_doubles_percent_signs() {
    let mut request = ApiRequest::new("GET", "https://x/?q=50%");
    request.name = "Half".into();
    let items = [Item {
        id: "half.http".into(),
        request,
    }];
    let out = export("All", "", &items, Format::Curl, Shell::Cmd);
    assert_eq!(
        out.text,
        "@echo off
REM All: 1 request from rBrowser's API client.
REM File paths are relative to .rbrowser/api.

REM Half
curl --location ^\"https://x/?q=50%%^\"
"
    );
}

#[test]
fn file_names_by_format_and_shell() {
    assert_eq!(
        file_name("shop", Format::Postman, Shell::Posix),
        "shop.postman_collection.json"
    );
    assert_eq!(file_name("shop", Format::Http, Shell::Posix), "shop.http");
    assert_eq!(file_name("shop", Format::Curl, Shell::Posix), "shop.sh");
    assert_eq!(
        file_name("shop", Format::Curl, Shell::PowerShell),
        "shop.ps1"
    );
    assert_eq!(file_name("shop", Format::Curl, Shell::Cmd), "shop.cmd");
}
