use std::collections::VecDeque;

use embedded_graphics::{
    mono_font::{self, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::{Dimensions, Point, Size},
    primitives::{Line, Primitive, PrimitiveStyle, Rectangle, Triangle},
    text::{Alignment, Baseline, Text, TextStyleBuilder},
    Drawable, Pixel,
};

use num::{rational::Ratio, Num};

use ssd1309::prelude::GraphicsMode;

use crate::controller::{DisplayCommand, SelectedParameter};

#[allow(unused)]
use crate::support::print_time_of;

const MIN_DYNAMIC_RANGE: f32 = 10.0;

pub fn dispaly_thread<DI>(
    mut disp: GraphicsMode<DI>,
    disp_channel: crossbeam::channel::Receiver<DisplayCommand>,
) -> !
where
    DI: display_interface::WriteOnlyDataCommand,
{
    let sz = disp.get_dimensions().0.into();
    let mut history = VecDeque::with_capacity(sz);

    let mut f_fistory = Vec::new();

    loop {
        match disp_channel.recv() {
            Ok(DisplayCommand::TitleScreen { option, selected }) => {
                draw_title_screen(&mut disp, option, selected)
            }
            Ok(DisplayCommand::SetupMenu { values, selected }) => {
                draw_menu(&mut disp, values, selected)
            }
            Ok(DisplayCommand::Measure { f, p, threashold }) => {
                match draw_measure(&mut disp, f, p, threashold, &mut history, f_fistory) {
                    Ok(h) => {
                        f_fistory = h;
                        Ok(())
                    }
                    Err(e) => panic!("{:?}", e),
                }
            }
            Ok(DisplayCommand::Result { f, p, threashold }) => {
                match draw_result(&mut disp, f, p, threashold, f_fistory) {
                    Ok(h) => {
                        f_fistory = h;
                        Ok(())
                    }
                    Err(e) => panic!("{:?}", e),
                }
            }
            Err(e) => {
                println!("Display cmd recive error: {}", e);
                Ok(())
            }
        }
        .expect("Failed to draw frame");
    }
}

fn draw_title_screen<DI>(
    display: &mut GraphicsMode<DI>,
    text: &'static str,
    selected: bool,
) -> Result<(), display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    display.clear();

    let big_font = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    let small_font_italic = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13_ITALIC)
        .text_color(BinaryColor::On)
        .build();

    let (display_w, _display_h) = {
        let d = display.get_dimensions();
        (d.0 as i32, d.1 as i32)
    };

    Text::with_baseline("Мини-Альфа", Point::new(18, -3), big_font, Baseline::Top).draw(display)?;
    Text::with_alignment(
        "v2.0",
        Point::new(
            (display_w / 2).into(),
            big_font.font.character_size.height as i32 + 8,
        ),
        big_font,
        Alignment::Center,
    )
    .draw(display)?;

    let text_w = {
        let mt = Text::with_baseline(text, Point::zero(), big_font, Baseline::Top);
        mt.bounding_box().size.width / 2 /* russian */
    } as i32;
    let begin_text = Text::with_baseline(
        text,
        Point::new(display_w / 2 - text_w / 2, 31),
        {
            if selected {
                MonoTextStyleBuilder::from(&big_font)
                    .background_color(BinaryColor::On)
                    .text_color(BinaryColor::Off)
                    .build()
            } else {
                big_font
            }
        },
        Baseline::Top,
    );

    let button = gen_text_bounding_rect(&begin_text, true);

    draw_arrows_to_rect(
        display,
        &button,
        5,
        -1,
        5,
        PrimitiveStyle::with_fill(BinaryColor::On),
    )?;

    button
        .into_styled(if selected {
            PrimitiveStyle::with_fill(BinaryColor::On)
        } else {
            PrimitiveStyle::with_stroke(BinaryColor::On, 1)
        })
        .draw(display)?;

    begin_text.draw(display)?;

    Rectangle::new(
        Point::new(
            0,
            display.get_dimensions().1 as i32 - small_font_italic.font.character_size.height as i32
                + 1,
        ),
        Size::new(
            display_w as u32,
            small_font_italic.font.character_size.height - 1,
        ),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(display)?;

    Text::new(
        "СКТБ ЭлПА(c)",
        Point::new(
            (Ratio::<i32>::new(1, 4) * display_w).to_integer() as i32,
            display.get_dimensions().1 as i32 - 2,
        ),
        MonoTextStyleBuilder::from(&small_font_italic)
            .background_color(BinaryColor::On)
            .text_color(BinaryColor::Off)
            .build(),
    )
    .draw(display)?;

    display.flush()
}

/// ```norun
/// Порог      1 mmHg
/// Период     100 мс
/// Сохранить и выйти
///
///      Настройки
/// ```
fn draw_menu<DI>(
    display: &mut GraphicsMode<DI>,
    values: super::controller::Parameters,
    selected_parameter: SelectedParameter,
) -> Result<(), display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    display.clear();

    let small_font_italic = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13_ITALIC)
        .text_color(BinaryColor::On)
        .build();
    let small_font = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13)
        .text_color(BinaryColor::On)
        .build();
    let small_font_selected = MonoTextStyleBuilder::from(&small_font)
        .background_color(BinaryColor::On)
        .text_color(BinaryColor::Off)
        .build();

    let (display_w, _display_h) = {
        let d = display.get_dimensions();
        (d.0 as i32, d.1 as i32)
    };

    const ITEMS_Y_OFFSET: i32 = 0;
    const LINE_SHIFT: i32 = 0;

    //-------------------------------------------------------------------------

    let pos = Text::with_baseline(
        " Порог ",
        Point::new(5, ITEMS_Y_OFFSET),
        if selected_parameter == SelectedParameter::Threshold {
            small_font_selected
        } else {
            small_font
        },
        Baseline::Top,
    )
    .draw(display)?;

    let tv = format!("{:0.0} mmHg", values.threshold);
    let value = Text::with_text_style(
        tv.as_str(),
        Point::new(display_w - 10, pos.y),
        small_font,
        TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Top)
            .build(),
    );

    if selected_parameter == SelectedParameter::Threshold {
        let rect = gen_text_bounding_rect(&value, false);
        draw_arrows_to_rect(
            display,
            &rect,
            3,
            -1,
            2,
            PrimitiveStyle::with_fill(BinaryColor::On),
        )?;
        rect.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
    }

    value.draw(display)?;

    //-------------------------------------------------------------------------

    let pos = Text::with_baseline(
        " Интервал ",
        Point::new(
            5,
            ITEMS_Y_OFFSET + (small_font.font.character_size.height as i32 + LINE_SHIFT),
        ),
        if selected_parameter == SelectedParameter::UpdatePeriodMs {
            small_font_selected
        } else {
            small_font
        },
        Baseline::Top,
    )
    .draw(display)?;

    let uv = format!("{} ms", values.update_period_ms);
    let value = Text::with_text_style(
        uv.as_str(),
        Point::new(display_w - 10, pos.y),
        small_font,
        TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Top)
            .build(),
    );

    if selected_parameter == SelectedParameter::UpdatePeriodMs {
        let rect = gen_text_bounding_rect(&value, false);
        draw_arrows_to_rect(
            display,
            &rect,
            3,
            -1,
            2,
            PrimitiveStyle::with_fill(BinaryColor::On),
        )?;
        rect.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
    }

    value.draw(display)?;

    //-------------------------------------------------------------------------

    let pos = Text::with_baseline(
        " Датчик ",
        Point::new(
            5,
            ITEMS_Y_OFFSET + (small_font.font.character_size.height as i32 + LINE_SHIFT) * 2,
        ),
        if selected_parameter == SelectedParameter::PSensorSelect {
            small_font_selected
        } else {
            small_font
        },
        Baseline::Top,
    )
    .draw(display)?;

    let sens_name = if values.try_use_alternative_sensor {
        "THYRACON"
    } else {
        "SCTB"
    };
    let value = Text::with_text_style(
        sens_name,
        Point::new(display_w - 10, pos.y),
        small_font,
        TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Top)
            .build(),
    );

    if selected_parameter == SelectedParameter::PSensorSelect {
        let rect = gen_text_bounding_rect(&value, false);
        draw_arrows_to_rect(
            display,
            &rect,
            3,
            -1,
            2,
            PrimitiveStyle::with_fill(BinaryColor::On),
        )?;
        rect.into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)?;
    }

    value.draw(display)?;

    //-------------------------------------------------------------------------

    Text::with_baseline(
        " Сохранить и выйти ",
        Point::new(
            5,
            ITEMS_Y_OFFSET + (small_font.font.character_size.height as i32 + LINE_SHIFT) * 3,
        ),
        if selected_parameter == SelectedParameter::SaveAndExit {
            small_font_selected
        } else {
            small_font
        },
        Baseline::Top,
    )
    .draw(display)?;

    Rectangle::new(
        Point::new(
            0,
            display.get_dimensions().1 as i32 - small_font_italic.font.character_size.height as i32
                + 1,
        ),
        Size::new(
            display_w as u32,
            small_font_italic.font.character_size.height - 1,
        ),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(display)?;

    Text::new(
        "Настройки",
        Point::new(
            (Ratio::<i32>::new(1, 3) * display_w).to_integer() as i32,
            display.get_dimensions().1 as i32 - 2,
        ),
        MonoTextStyleBuilder::from(&small_font_italic)
            .background_color(BinaryColor::On)
            .text_color(BinaryColor::Off)
            .build(),
    )
    .draw(display)?;

    display.flush()
}

/// ```norun
/// P: 123.456      F: 123.456
/// <график Y=[P[0]...Threshhold]>
/// __________________________
/// ```
fn draw_measure<DI>(
    display: &mut GraphicsMode<DI>,
    f: Option<f32>,
    p: Option<f32>,
    threashold: f32,
    history: &mut VecDeque<f32>,
    mut f_history: Vec<(f32, f32)>,
) -> Result<Vec<(f32, f32)>, display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    display.clear();

    let small_font = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13)
        .text_color(BinaryColor::On)
        .build();

    let (display_w, display_h) = {
        let d = display.get_dimensions();
        (d.0 as i32, d.1 as i32)
    };

    if f.is_none() && p.is_none() {
        // reset
        history.clear();
        f_history.clear();
    } else {
        if history.len() == display_w as usize {
            history.pop_front();
        }

        let p = p.unwrap_or_default();
        history.push_back(p);
        f_history.push((p, f.unwrap_or_default()));

        if f_history.len() > 1000 {
            f_history = f_history[500..].to_vec();
        }
    }

    let mut range = history
        .iter()
        .max_by_key(|v| ordered_float::OrderedFloat(**v))
        .unwrap_or(&800.0)
        - threashold;

    if range < MIN_DYNAMIC_RANGE {
        range = MIN_DYNAMIC_RANGE;
    }

    let max_y = display_h as u32 - 20;

    let line_style = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    for line_n in 0..display_w {
        if let Some(element) = history.get(line_n as usize) {
            let stroke_len = transform_size(*element, max_y as f32, range) as i32;
            Line::new(
                Point::new(line_n, display_h - stroke_len),
                Point::new(line_n, display_h),
            )
            .into_styled(line_style)
            .draw(display)?;
        } else {
            Line::new(
                Point::new(0, display_h - 1),
                Point::new(display_w, display_h - 1),
            )
            .into_styled(line_style)
            .draw(display)?;

            break;
        }
    }

    Text::with_text_style(
        format!("P: {:0.2}", p.unwrap_or(f32::NAN)).as_str(),
        Point::new(2, 2),
        small_font,
        TextStyleBuilder::new()
            .alignment(Alignment::Left)
            .baseline(Baseline::Top)
            .build(),
    )
    .draw(display)?;

    Text::with_text_style(
        format!("F: {:0.2}", f.unwrap_or(f32::NAN)).as_str(),
        Point::new(display_w - 2, 2),
        small_font,
        TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Top)
            .build(),
    )
    .draw(display)?;

    display.flush()?;

    Ok(f_history)
}

fn draw_result<DI>(
    display: &mut GraphicsMode<DI>,
    f: f32,
    p: f32,
    _threashold: f32,
    f_history: Vec<(f32, f32)>,
) -> Result<Vec<(f32, f32)>, display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    display.clear();

    let small_font = MonoTextStyleBuilder::new()
        .font(&mono_font::iso_8859_5::FONT_6X13)
        .text_color(BinaryColor::On)
        .build();

    let (display_w, display_h) = {
        let d = display.get_dimensions();
        (d.0 as i32, d.1 as i32)
    };

    let pos = Text::with_baseline("Давление:", Point::new(1, 2), small_font, Baseline::Top)
        .draw(display)?;

    let select_mapped_point = |line_n: u32| {
        transform_size(line_n, f_history.len() as u32 - 1, display_w as u32 - 1) as usize
    };

    let mapped_height = |h_e: usize, range_min: f32, range_max: f32, max_y: u32| {
        transform_size(
            f_history[h_e].1 - range_min,
            max_y as f32,
            range_max - range_min,
        ) as i32
    };

    Text::with_text_style(
        format!("{:0.02} mmHg", p).as_str(),
        Point::new(display_w - 1, pos.y),
        small_font,
        TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Top)
            .build(),
    )
    .draw(display)?;

    let pos = Text::with_baseline(
        "Частота:",
        Point::new(1, (small_font.font.character_size.height + 1) as i32),
        small_font,
        Baseline::Top,
    )
    .draw(display)?;

    Text::with_text_style(
        format!("{:0.02} Hz", f).as_str(),
        Point::new(display_w - 1, pos.y),
        small_font,
        TextStyleBuilder::new()
            .alignment(Alignment::Right)
            .baseline(Baseline::Top)
            .build(),
    )
    .draw(display)?;

    // graph
    let max_y = display_h as u32 - 20;

    let (mut range_min, mut range_max) = f_history
        .iter()
        .fold((f32::INFINITY, -f32::INFINITY), |acc, &p| {
            (acc.0.min(p.1), acc.1.max(p.1))
        });

    const MIN_DIFF: f32 = 1.0;
    #[allow(deprecated)]
    if range_min.abs_sub(range_max) < MIN_DIFF {
        range_min -= MIN_DIFF / 2.0;
        range_max += MIN_DIFF / 2.0;
    }

    println!("Results plot range: {:?}", (range_min, range_max));

    // в начале измерения происходит резкий скачек, поэтому начальные точки погут быть нерелевантны
    // необходимо обойти f_history сзади и найти номер последней точки, где сограняется устйчивай спад значений
    let mut last_p = f_history.last().map(|p| p.0).unwrap_or(0.0);
    let mut max_p_index = 0;
    for (i, (p, _)) in f_history.iter().enumerate().rev() {
        if *p < last_p {
            max_p_index = i;
            break;
        } else {
            last_p = *p;
        };
    }

    let thrend = crate::linear_regression::linear_regression(&f_history[max_p_index..]);

    for line_n in 0..display_w as u32 {
        let h_e = select_mapped_point(line_n);
        let stroke_len = mapped_height(h_e, range_min, range_max, max_y);
        Pixel(
            Point::new(line_n as i32, display_h - 1 - stroke_len),
            BinaryColor::On,
        )
        .draw(display)?;
    }

    // thrend
    Line::new(
        Point::new(
            0,
            display_h
                - 1
                - transform_size(
                    thrend.calc(select_mapped_point(0) as f32) - range_min,
                    max_y as f32,
                    range_max - range_min,
                ) as i32,
        ),
        Point::new(
            display_w - 1,
            display_h
                - 1
                - transform_size(
                    thrend.calc(select_mapped_point(display_w as u32 - 1) as f32) - range_min,
                    max_y as f32,
                    range_max - range_min,
                ) as i32,
        ),
    )
    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
    .draw(display)?;

    Text::with_baseline(
        format!(" Чувст.: {:.01} Hz/mmHg ", thrend.k).as_str(),
        Point::new(2, display_h),
        MonoTextStyleBuilder::from(&small_font)
            .background_color(BinaryColor::On)
            .text_color(BinaryColor::Off)
            .build(),
        Baseline::Bottom,
    )
    .draw(display)?;

    display.flush()?;

    Ok(Vec::new()) // clear history
}

fn gen_text_bounding_rect<T: Dimensions>(text: &T, is_russian_text: bool) -> Rectangle {
    let mut bb = text.bounding_box();

    if is_russian_text {
        bb.size.width /= 2;
    }
    bb.top_left.x -= 2;
    bb.size.width += 3 * 2;

    bb
}

fn draw_arrows_to_rect<DI>(
    display: &mut GraphicsMode<DI>,
    rect: &Rectangle,
    arrow_len: i32,
    vertical_resize: i32,
    h_offset: i32,
    style: PrimitiveStyle<BinaryColor>,
) -> Result<(), display_interface::DisplayError>
where
    DI: display_interface::WriteOnlyDataCommand,
{
    let center_line = rect.center().y;
    let button_heigh_half = (rect.size.height / 2) as i32;
    let left = rect.top_left.x;
    let right = rect.bottom_right().map_or_else(|| left, |p| p.x);

    Triangle::new(
        Point::new(left - h_offset - arrow_len, center_line),
        Point::new(
            left - h_offset,
            center_line - (button_heigh_half + vertical_resize),
        ),
        Point::new(
            left - h_offset,
            center_line + (button_heigh_half + vertical_resize),
        ),
    )
    .into_styled(style)
    .draw(display)?;
    Triangle::new(
        Point::new(right + h_offset + arrow_len, center_line),
        Point::new(
            right + h_offset,
            center_line - (button_heigh_half + vertical_resize),
        ),
        Point::new(
            right + h_offset,
            center_line + (button_heigh_half + vertical_resize),
        ),
    )
    .into_styled(style)
    .draw(display)?;

    Ok(())
}

fn transform_size<T: Num>(current: T, target_max: T, max_value: T) -> T {
    (target_max * current) / max_value
}
