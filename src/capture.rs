use image::RgbaImage;

pub fn capture_fullscreen() -> Result<RgbaImage, Box<dyn std::error::Error>> {
    use xcap::Monitor;

    // 获取所有显示器
    let monitors = Monitor::all()?;

    if monitors.is_empty() {
        return Err("No monitors found".into());
    }

    // 如果有多个显示器，我们可以合并它们，这里先处理主显示器
    let primary_monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap())
        .or_else(|| monitors.first())
        .ok_or("No monitor available")?;

    println!(
        "Capturing monitor: {}x{}",
        primary_monitor.width().unwrap(),
        primary_monitor.height().unwrap()
    );

    // 捕获显示器截图
    let image_buffer = primary_monitor.capture_image()?;

    // 将 xcap 的 ImageBuffer 转换为 image crate 的 RgbaImage
    let img = RgbaImage::from_raw(
        image_buffer.width(),
        image_buffer.height(),
        image_buffer.into_raw(),
    )
    .ok_or("Failed to create image from raw data")?;

    Ok(img)
}

// 获取所有显示器的合并截图（用于多显示器支持）
// pub fn capture_all_monitors() -> Result<RgbaImage, Box<dyn std::error::Error>> {
//     use xcap::{Monitor, Window};
//     use image::ImageBuffer;

//     let monitors = Monitor::all()?;

//     if monitors.is_empty() {
//         return Err("No monitors found".into());
//     }

//     // 计算所有显示器的总边界
//     let mut min_x = i32::MAX;
//     let mut min_y = i32::MAX;
//     let mut max_x = i32::MIN;
//     let mut max_y = i32::MIN;

//     for monitor in &monitors {
//         let x = monitor.x().unwrap();
//         let y = monitor.y().unwrap();
//         min_x = min_x.min(x);
//         min_y = min_y.min(y);
//         max_x = max_x.max(x + monitor.width().unwrap() as i32);
//         max_y = max_y.max(y + monitor.height().unwrap() as i32);
//     }

//     let total_width = (max_x - min_x) as u32;
//     let total_height = (max_y - min_y) as u32;

//     println!("Total capture area: {}x{}", total_width, total_height);

//     // 创建合并图像
//     let mut combined_image = RgbaImage::new(total_width, total_height);

//     for monitor in &monitors {
//         let monitor_image = monitor.capture_image()?;
//         let monitor_x = monitor.x().unwrap() - min_x;
//         let monitor_y = monitor.y().unwrap() - min_y;

//         // 将每个显示器的截图放到正确的位置
//         for y in 0..monitor.height().unwrap() {
//             for x in 0..monitor.width().unwrap() {
//                 let src_idx = (y * monitor.width().unwrap() + x) as usize * 4;
//                 let pixel = &monitor_image.bytes()[src_idx..src_idx + 4];

//                 let combined_x = monitor_x as u32 + x as u32;
//                 let combined_y = monitor_y as u32 + y as u32;

//                 if combined_x < total_width && combined_y < total_height {
//                     combined_image.put_pixel(combined_x, combined_y,
//                         image::Rgba([pixel[0], pixel[1], pixel[2], pixel[3]]));
//                 }
//             }
//         }
//     }

//     Ok(combined_image)
// }

// 根据矩形裁剪图像
#[allow(dead_code)]
pub fn crop_image(
    image: &RgbaImage,
    rect: (i32, i32, i32, i32),
) -> Result<RgbaImage, Box<dyn std::error::Error>> {
    let (x1, y1, x2, y2) = rect;

    // 确保坐标有效
    let x1 = x1.max(0) as u32;
    let y1 = y1.max(0) as u32;
    let x2 = x2.min(image.width() as i32) as u32;
    let y2 = y2.min(image.height() as i32) as u32;

    if x1 >= x2 || y1 >= y2 {
        return Err("Invalid crop region".into());
    }

    let width = x2 - x1;
    let height = y2 - y1;

    let mut cropped = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x1 + x, y1 + y);
            cropped.put_pixel(x, y, *pixel);
        }
    }

    println!("Cropped image: {}x{}", width, height);

    Ok(cropped)
}
