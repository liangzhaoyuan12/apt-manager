use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::gdk;

/// 由 desktop 的 Icon 字段构造图标 Image(绝对路径或图标名)。
/// 返回 None 表示无可用图标,调用方应隐藏占位。
pub fn image_for(icon: &str, pixel_size: i32) -> Option<gtk::Image> {
    let img = gtk::Image::new();
    if set_image(&img, icon, pixel_size) {
        Some(img)
    } else {
        None
    }
}

/// 把 desktop Icon 字段的内容设置到已有 Image 上,返回是否有图标。
///
/// - 绝对路径:GdkPixbuf 按 pixel_size 等比缩放后转 texture,带缓存
///   (翻页/重渲染不重复解码;解码失败同样缓存,避免反复读坏文件)。
/// - 图标名:交给 GTK 图标主题按主题与 pixel_size 解析。
pub fn set_image(img: &gtk::Image, icon: &str, pixel_size: i32) -> bool {
    img.set_pixel_size(pixel_size);
    if icon.is_empty() {
        return false;
    }
    if icon.starts_with('/') {
        let Some(tex) = texture_for(icon, pixel_size) else {
            return false;
        };
        // texture 已缩放为 pixel_size,Image 按原始像素绘制即可
        img.set_paintable(Some(tex.as_ref()));
        return true;
    }
    img.set_icon_name(Some(icon));
    true
}

/// 绝对路径图标 → 已缩放 texture 的缓存(key = 路径 + 目标尺寸)
fn texture_for(path: &str, pixel_size: i32) -> Option<Rc<gdk::Texture>> {
    thread_local! {
        static CACHE: RefCell<HashMap<(String, i32), Option<Rc<gdk::Texture>>>> =
            RefCell::new(HashMap::new());
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let key = (path.to_string(), pixel_size);
        if let Some(hit) = c.get(&key) {
            return hit.clone();
        }
        // at_size 保持宽高比,长边缩放到 pixel_size
        let loaded = gdk_pixbuf::Pixbuf::from_file_at_size(path, pixel_size, pixel_size)
            .ok()
            .map(|pb| Rc::new(gdk::Texture::for_pixbuf(&pb)));
        c.insert(key, loaded.clone());
        loaded
    })
}
