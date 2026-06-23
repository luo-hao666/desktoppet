//! DOCX / PPTX 文本提取
//! - DOCX: 解 ZIP → 读 word/document.xml → 收集 <w:t> 文本
//! - PPTX: 解 ZIP → 遍历 ppt/slides/slide*.xml → 收集 <a:t> 文本

use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::Read;
use zip::ZipArchive;

pub fn extract_office_text(path: &str, ext: &str) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("解压失败: {}", e))?;

    match ext {
        "docx" => {
            let mut doc = archive
                .by_name("word/document.xml")
                .map_err(|e| format!("找不到文档内容: {}", e))?;
            let mut xml = String::new();
            doc.read_to_string(&mut xml)
                .map_err(|e| format!("读取失败: {}", e))?;
            extract_text_from_docx_xml(&xml)
        }
        "pptx" => extract_text_from_pptx(&mut archive),
        _ => Err("不支持的格式".to_string()),
    }
}

fn extract_text_from_docx_xml(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut text = String::new();
    let mut buf = Vec::new();
    let mut in_w_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"w:t" {
                    in_w_t = true;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"w:t" {
                    in_w_t = false;
                }
                if e.name().as_ref() == b"w:p" {
                    text.push('\n');
                }
            }
            Ok(Event::Text(ref e)) if in_w_t => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("XML 解析错误: {}", err)),
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

fn extract_text_from_pptx(archive: &mut ZipArchive<File>) -> Result<String, String> {
    let mut all_text = String::new();
    for i in 1..=200 {
        let path = format!("ppt/slides/slide{}.xml", i);
        match archive.by_name(&path) {
            Ok(mut file) => {
                let mut xml = String::new();
                if file.read_to_string(&mut xml).is_ok() {
                    let slide_text = extract_text_from_pptx_xml(&xml)?;
                    if !slide_text.trim().is_empty() {
                        all_text.push_str(&format!("[幻灯片 {}]\n{}\n\n", i, slide_text));
                    }
                }
            }
            Err(_) => break,
        }
    }
    Ok(all_text)
}

fn extract_text_from_pptx_xml(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut text = String::new();
    let mut buf = Vec::new();
    let mut in_a_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if e.name().as_ref() == b"a:t" {
                    in_a_t = true;
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"a:t" {
                    in_a_t = false;
                }
                if e.name().as_ref() == b"a:p" {
                    text.push('\n');
                }
            }
            Ok(Event::Text(ref e)) if in_a_t => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("XML 解析错误: {}", err)),
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}
