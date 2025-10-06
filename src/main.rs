use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::list::ListObjectsRequest;
use log::{LevelFilter, error, info};
use std::convert::Infallible;
use std::env;
use time::format_description;
use warp::http::StatusCode;
use warp::{Filter, http::Response};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info)
        .init();
    info!("GCS Web Server starting...");
    let args: Vec<String> = env::args().collect();
    let bucket_name = match args.get(1) {
        Some(bucket) => bucket.to_string(),
        None => panic!("No bucket was passed"),
    };
    info!("Connecting to bucket gs://{bucket_name}");

    let config = ClientConfig::default().with_auth().await?;
    let client = Client::new(config);

    let client_filter = warp::any().map(move || client.clone());
    let bucket_filter = warp::any().map(move || bucket_name.clone());

    let routes = warp::path::full()
        .and(client_filter)
        .and(bucket_filter)
        .and_then(serve_gcs_content);

    warp::serve(routes).run(([0, 0, 0, 0], 8585)).await;

    Ok(())
}

async fn check_for_dirs(
    client: &Client,
    bucket: String,
    path: String,
) -> Option<Response<Vec<u8>>> {
    if path.ends_with('/') || path.is_empty() {
        return None;
    }
    let list_req_test_prefix = ListObjectsRequest {
        bucket,
        prefix: Some(format!("{}/", path)),
        delimiter: Some("/".to_string()),
        max_results: Some(1), // Fast check: only need 1 result to confirm existence
        ..Default::default()
    };
    match client.list_objects(&list_req_test_prefix).await {
        Ok(res) => {
            // If any prefixes (sub-folders) or items (files) exist, it's a folder.
            if res.prefixes.is_some() || res.items.is_some() {
                let new_path = format!("/{}/", path);
                info!("REDIRECT: path='{}' -> '{}'", path, new_path);

                // Return a 302 Found response to redirect the browser
                let res = Response::builder()
                    .status(StatusCode::FOUND)
                    .header("Location", new_path)
                    .body(Vec::new())
                    .unwrap();
                return Some(res);
            }
        }
        Err(e) => {
            error!("REDIRECT_CHECK_ERROR: path='{}', error='{:?}'", path, e);
        }
    }
    None
    // client
    //     .list_objects(&list_req_test_prefix)
    //     .await
    //     .is_ok_and(|res| res.prefixes.is_some() || res.items.is_some())
}

async fn serve_gcs_content(
    path: warp::path::FullPath,
    client: Client,
    bucket_name: String,
) -> Result<impl warp::Reply, Infallible> {
    let path_str = path.as_str().trim_start_matches('/').to_string();

    // The GCS API is a flat hierarchy. A "folder" is just an object prefix.
    let is_dir = path_str.ends_with('/');

    // Check if the path is a file by attempting to get its metadata.
    let file_req = GetObjectRequest {
        bucket: bucket_name.clone(),
        object: path_str.clone(),
        ..Default::default()
    };
    let file_metadata = client.get_object(&file_req).await;

    // Check if a path without a trailing slash is a directory
    if let Some(res) = check_for_dirs(&client, bucket_name.clone(), path_str.clone()).await {
        return Ok(res);
    };

    // If metadata is found, it's a file. Serve the download.
    if file_metadata.is_ok() {
        let request = GetObjectRequest {
            bucket: bucket_name,
            object: path_str.clone(),
            ..Default::default()
        };
        info!("DOWNLOAD_REQUEST: path='{}'", path_str);

        let response = match client.download_object(&request, &Range::default()).await {
            Ok(data) => {
                let filename = path_str.split('/').next_back().unwrap_or("file");

                // Get content type, diplay text files for config.txt
                let content_type = file_metadata
                    .unwrap()
                    .content_type
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let disposition = if content_type.starts_with("text/")
                    || filename.ends_with("env")
                    || filename.ends_with("list")
                {
                    "inline"
                } else {
                    "attachment"
                };
                info!(
                    "DOWNLOAD_SUCCESS: file='{}', size={} bytes, disposition='{}'",
                    path_str,
                    data.len(),
                    disposition
                );
                Response::builder()
                    .header(
                        "Content-Disposition",
                        format!("{disposition}; filename=\"{filename}\""),
                    )
                    .body(data)
                    .unwrap()
            }
            Err(e) => {
                error!("DOWNLOAD_ERROR: file='{}', error='{:?}'", path_str, e);
                Response::builder()
                    .status(404)
                    .body("File not found".as_bytes().to_vec())
                    .unwrap()
            }
        };
        return Ok(response);
    }

    // If it's a directory (either with or without a trailing slash), list its contents
    if is_dir || path_str.is_empty() {
        let prefix = if path_str.is_empty() {
            None
        } else {
            Some(format!("{}/", path_str.trim_end_matches('/')))
        };

        info!("LISTING_REQUEST: path='{}'", path_str);
        let mut folders = Vec::new();
        let mut files = Vec::new();
        let mut next_page_token: Option<String> = None;

        // Currently we have folders with 10,000 files. So we have to loop this till we have
        // no more page tokens.
        loop {
            let list_req = ListObjectsRequest {
                bucket: bucket_name.clone(),
                prefix: prefix.clone(),
                delimiter: Some("/".to_string()),
                max_results: Some(2500),
                page_token: next_page_token,
                ..Default::default()
            };
            let objects = match client.list_objects(&list_req).await {
                Ok(objects) => objects,
                Err(e) => {
                    eprintln!("Error listing objects: {:?}", e);
                    return Ok(Response::builder()
                        .status(500)
                        .body("Error listing content".as_bytes().to_vec())
                        .unwrap());
                }
            };
            if objects.prefixes.is_some() && list_req.page_token.is_none() {
                for prefix in objects.prefixes.unwrap() {
                    folders.push(prefix);
                }
            }

            if let Some(items) = objects.items {
                for item in items {
                    // Do not include the directory itself in the file list
                    if item.name != path_str {
                        let size = item.size;
                        let filesize: String;
                        if size > 1048576 {
                            filesize = format!("{}M", (size as f64 / 1048576_f64).round())
                        } else if size > 1024 {
                            filesize = format!("{}K", (size as f64 / 1024_f64).round())
                        } else {
                            filesize = format!("{}", size)
                        }
                        if let Some(updated) = item.updated {
                            let format =
                                format_description::parse("[year]-[month]-[day] [hour]:[minute]")
                                    .unwrap();
                            let u = updated.format(&format).unwrap();
                            files.push((item.name, filesize, u));
                        }
                    }
                }
            }
            if objects.next_page_token.is_none() {
                break;
            }
            next_page_token = objects.next_page_token;
        }
        info!(
            "LISTING_SUCCESS: path='{}', folders={}, files={}, total={}",
            path_str,
            folders.len(),
            files.len(),
            folders.len() + files.len()
        );

        let html = build_html(path_str, folders, files);

        return Ok(Response::builder()
            .header("Content-Type", "text/html")
            .body(html.into_bytes())
            .unwrap());
    }

    // If it is neither a file nor a directory
    Ok(Response::builder()
        .status(404)
        .body("Not Found".as_bytes().to_vec())
        .unwrap())
}
fn build_html(
    path_str: String,
    folders: Vec<String>,
    files: Vec<(String, String, String)>,
) -> String {
    let parent_path = if path_str.is_empty() {
        "".to_string()
    } else if path_str.ends_with("/") {
        str::trim_end_matches(&path_str, '/')
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent)
            .to_string()
    } else {
        path_str
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent)
            .to_string()
    };
    format!(
            r#"
<!DOCTYPE html>
<html>
<head>
    <title>Index of {}</title>
    <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.4.0/css/all.min.css">
    <style>td {{
        padding:2px;
    }}</style>
</head>
<body>
    <h1>Index of {}</h1>
    <hr>
    <table style="">
    <tr>
    <th valign="top"></th>
    <th>Name</th>
    <th>Size</th>
    <th>Last modified</th>
    </tr>
    {}
    {}
    {}
    </table>
    <hr>
</body>
</html>
"#,
            (if path_str.is_empty() {
                "/".to_string()
            } else {
                path_str.clone()
            }),
            (if path_str.is_empty() {
                "/".to_string()
            } else {
                path_str.clone()
            }),
            (if path_str.is_empty() {
                "".to_string()
            } else {
                format!("<tr><td><a href=\"/{}\">../</a></td></tr>", parent_path)
            }),
            folders
                .iter()
                .map(|f| format!(
                    "<tr><td><i class=\"fa-solid fa-folder\"></i></td> <td><a href=\"/{}\">{}</a></td><td align=\"right\">-</td><td align=\"right\">-</td></tr>",
                    f,
                    f.trim_end_matches('/').split('/').next_back().unwrap_or("")
                ))
                .collect::<String>(),
            files
                .iter()
                .map(|(name, size, updated)| format!(
                    r#"<tr><td><i class="fa-solid fa-file"></i></td> <td><a href="/{}">{}</a></td><td align="right">{}</td><td align="right">{}</td></tr>"#,
                    name,
                    name.split('/').next_back().unwrap_or(""),
                    size,
                    updated
                ))
                .collect::<String>()
        )
}
