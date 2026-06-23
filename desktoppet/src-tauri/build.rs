use std::path::Path;

fn main() {
    tauri_build::build();

    // ort crate 的 load-dynamic 模式需要 onnxruntime.dll 在 exe 目录下。
    // download-binaries 无法与 load-dynamic 同时工作（后者会设置 disable-linking
    // 导致构建脚本跳过下载），因此手动下载兼容版本的 DLL。
    let _manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir_str = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);
    // OUT_DIR = <target>/<profile>/build/<crate>-<hash>/out → 向上 3 级到 profile 目录
    let profile_dir = match out_dir.ancestors().nth(3) {
        Some(p) => p.to_path_buf(),
        None => return,
    };

    let dll_dst = profile_dir.join("onnxruntime.dll");
    let providers_dst = profile_dir.join("onnxruntime_providers_shared.dll");

    // 如果 DLL 已存在则跳过下载
    if dll_dst.exists() && providers_dst.exists() {
        return;
    }

    // ONNX Runtime 1.24.2, Windows x64
    let zip_url = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-1.24.2.zip";
    let zip_path = profile_dir.join("onnxruntime.zip");
    let extract_dir = profile_dir.join("onnxruntime_extract");

    // 使用 PowerShell 下载
    let ps_script = format!(
        r#"
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
if (-not (Test-Path '{zip}')) {{
    Write-Host '[build.rs] Downloading ONNX Runtime 1.24.2...'
    Invoke-WebRequest -Uri '{url}' -OutFile '{zip}' -UseBasicParsing
}}
if (-not (Test-Path '{extract}')) {{
    Write-Host '[build.rs] Extracting...'
    Expand-Archive -Path '{zip}' -DestinationPath '{extract}' -Force
}}
$dll = Get-ChildItem -Path '{extract}' -Recurse -Filter 'onnxruntime.dll' | Select-Object -First 1
if ($dll) {{
    Copy-Item $dll.FullName '{dll_dst}'
    Write-Host '[build.rs] Copied onnxruntime.dll'
}}
$providers = Get-ChildItem -Path '{extract}' -Recurse -Filter 'onnxruntime_providers_shared.dll' | Select-Object -First 1
if ($providers) {{
    Copy-Item $providers.FullName '{providers_dst}'
    Write-Host '[build.rs] Copied onnxruntime_providers_shared.dll'
}}
# 清理临时文件
Remove-Item '{zip}' -Force -ErrorAction SilentlyContinue
Remove-Item '{extract}' -Recurse -Force -ErrorAction SilentlyContinue
Write-Host '[build.rs] Cleaned up temp files'
"#,
        zip = zip_path.display(),
        extract = extract_dir.display(),
        url = zip_url,
        dll_dst = dll_dst.display(),
        providers_dst = providers_dst.display(),
    );

    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_script])
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!("cargo:warning=Failed to download ONNX Runtime DLL: {stderr}");
                println!("cargo:warning=Please manually download from {zip_url} and place onnxruntime.dll in {}", profile_dir.display());
            }
        }
        Err(e) => {
            println!("cargo:warning=Failed to run PowerShell for ONNX Runtime download: {e}");
        }
    }
}
