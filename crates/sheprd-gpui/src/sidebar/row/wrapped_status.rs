use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hsla, InspectorElementId, IntoElement,
    LayoutId, Pixels, ShapedLine, SharedString, Style, TextAlign, TextRun, Window, point, px,
    relative, rgb, size,
};

/// Two-line wrapped status text whose first line shares horizontal space with a
/// leading prefix (`[icon] agent · `) and optional trailing pill (`[recap]`),
/// while its second line flows across the full width with trailing ellipsis.
pub(super) struct WrappedStatusText {
    pub(super) text: SharedString,
    pub(super) first_line_indent: Pixels,
    pub(super) first_line_trailing: Pixels,
    pub(super) font_size: Pixels,
    pub(super) line_height: Pixels,
    pub(super) color: u32,
}

pub(super) struct ShapedPair {
    line1: Option<(SharedString, ShapedLine)>,
    line2: Option<(SharedString, ShapedLine)>,
}

impl IntoElement for WrappedStatusText {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for WrappedStatusText {
    type RequestLayoutState = ();
    type PrepaintState = ShapedPair;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: size(relative(1.).into(), (self.line_height * 2.).into()),
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> ShapedPair {
        let total_w = bounds.size.width;
        if self.text.is_empty() || total_w <= px(0.) {
            return ShapedPair {
                line1: None,
                line2: None,
            };
        }

        let font = window.text_style().font();
        let color: Hsla = rgb(self.color).into();
        let run = |len: usize| TextRun {
            len,
            font: font.clone(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shape_one = |text: SharedString| {
            let r = run(text.len());
            let shaped = window
                .text_system()
                .shape_line(text.clone(), self.font_size, &[r], None);
            (text, shaped)
        };

        let clean: std::borrow::Cow<'_, str> = if self.text.contains(['\n', '\r']) {
            std::borrow::Cow::Owned(
                self.text
                    .chars()
                    .map(|c| if matches!(c, '\n' | '\r') { ' ' } else { c })
                    .collect(),
            )
        } else {
            std::borrow::Cow::Borrowed(self.text.as_ref())
        };
        let text_ref = clean.as_ref();

        let line1_w = (total_w - self.first_line_indent - self.first_line_trailing).max(px(0.));
        let (line1, rest) = if line1_w <= px(0.) {
            (None, text_ref)
        } else {
            let shared: SharedString = match &clean {
                std::borrow::Cow::Borrowed(_) => self.text.clone(),
                std::borrow::Cow::Owned(s) => SharedString::from(s.clone()),
            };
            let r = run(text_ref.len());
            let split = window
                .text_system()
                .shape_text(shared.clone(), self.font_size, &[r], Some(line1_w), Some(2))
                .ok()
                .and_then(|lines| lines.into_iter().next())
                .and_then(|first| {
                    let boundary = *first.wrap_boundaries().first()?;
                    let glyph = first
                        .runs()
                        .get(boundary.run_ix)?
                        .glyphs
                        .get(boundary.glyph_ix)?;
                    (glyph.index <= text_ref.len() && text_ref.is_char_boundary(glyph.index))
                        .then_some(glyph.index)
                });

            match split {
                None => {
                    let shaped = shape_one(shared);
                    if shaped.1.width <= line1_w {
                        (Some(shaped), "")
                    } else {
                        let cut = fit_char_end(text_ref, &shaped.1, line1_w);
                        let head = text_ref[..cut].trim_end();
                        let tail = text_ref[cut..].trim_start();
                        let l1 = (!head.is_empty()).then(|| shape_one(head.to_owned().into()));
                        (l1, tail)
                    }
                }
                Some(0) => (None, text_ref),
                Some(index) => {
                    let head = text_ref[..index].trim_end();
                    let tail = text_ref[index..].trim_start();
                    if head.is_empty() {
                        (None, tail)
                    } else {
                        let shaped = shape_one(head.to_owned().into());
                        if shaped.1.width <= line1_w {
                            (Some(shaped), tail)
                        } else {
                            let cut = fit_char_end(head, &shaped.1, line1_w);
                            let fitted = text_ref[..cut].trim_end();
                            let tail = text_ref[cut..].trim_start();
                            let l1 =
                                (!fitted.is_empty()).then(|| shape_one(fitted.to_owned().into()));
                            (l1, tail)
                        }
                    }
                }
            }
        };

        let line2 = if rest.is_empty() {
            None
        } else {
            let full = shape_one(rest.to_owned().into());
            if full.1.width <= total_w {
                Some(full)
            } else {
                let ellipsis = shape_one(SharedString::from("…"));
                let avail = (total_w - ellipsis.1.width).max(px(0.));
                let cut = fit_char_end(rest, &full.1, avail);
                let prefix = rest[..cut].trim_end().trim_end_matches('…');
                let ellipsized = format!("{prefix}…");
                Some(shape_one(ellipsized.into()))
            }
        };

        ShapedPair { line1, line2 }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        shaped: &mut ShapedPair,
        window: &mut Window,
        cx: &mut App,
    ) {
        let rtl = crate::direction::is_rtl();
        if let Some((_, line1)) = &shaped.line1 {
            let x = if rtl {
                bounds.right() - self.first_line_indent - line1.width
            } else {
                bounds.left() + self.first_line_indent
            };
            let _ = line1.paint(
                point(x, bounds.top()),
                self.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        if let Some((_, line2)) = &shaped.line2 {
            let x = if rtl {
                bounds.right() - line2.width
            } else {
                bounds.left()
            };
            let _ = line2.paint(
                point(x, bounds.top() + self.line_height),
                self.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }

        #[cfg(any(test, feature = "integration-test"))]
        {
            let mut rendered = String::new();
            let mut width = px(0.);
            if let Some((text, line1)) = &shaped.line1 {
                rendered.push_str(text);
                width = width.max(line1.width);
            }
            if let Some((text, line2)) = &shaped.line2 {
                if !rendered.is_empty() {
                    rendered.push('\n');
                }
                rendered.push_str(text);
                width = width.max(line2.width);
            }
            let clipped = shaped.line1.as_ref().is_some_and(|(_, l)| {
                l.width
                    > (bounds.size.width - self.first_line_indent - self.first_line_trailing)
                        .max(px(0.))
                        + px(0.5)
            }) || shaped
                .line2
                .as_ref()
                .is_some_and(|(_, l)| l.width > bounds.size.width + px(0.5));
            crate::sidebar::layout_tests::record_wrapped_probe(
                self.text.to_string(),
                bounds,
                window.content_mask().bounds,
                rendered,
                width,
                clipped,
                cx,
            );
        }
    }
}

/// Largest UTF-8 character boundary in `text` whose shaped right edge stays
/// within `max_x`.
fn fit_char_end(text: &str, shaped: &ShapedLine, max_x: Pixels) -> usize {
    let mut positions = Vec::new();
    for run in &shaped.runs {
        for glyph in &run.glyphs {
            positions.push((glyph.index, glyph.position.x));
        }
    }
    let mut end = 0;
    for i in 0..positions.len() {
        let next_x = positions.get(i + 1).map_or(shaped.width, |(_, x)| *x);
        if next_x <= max_x {
            let next_byte = positions.get(i + 1).map_or(text.len(), |(idx, _)| *idx);
            if next_byte <= text.len() && text.is_char_boundary(next_byte) {
                end = end.max(next_byte);
            }
        } else {
            break;
        }
    }
    end
}
