use egui::Color32;

pub fn blue_shift_color(c: Color32, dblue: i8) -> Color32 {
    let b = if dblue >= 0 {
        c.b().saturating_add(dblue as u8)
    } else {
        c.b().saturating_sub(-dblue as u8)
    };
    Color32::from_rgba_premultiplied(c.r(), c.g(), b, c.a())
}

pub trait SubsliceOffset {
    /**
    Returns the byte offset of an inner slice relative to an enclosing outer slice.

    Examples

    ```ignore
    let string = "a\nb\nc";
    let lines: Vec<&str> = string.lines().collect();
    assert!(string.subslice_offset_stable(lines[0]) == Some(0)); // &"a"
    assert!(string.subslice_offset_stable(lines[1]) == Some(2)); // &"b"
    assert!(string.subslice_offset_stable(lines[2]) == Some(4)); // &"c"
    assert!(string.subslice_offset_stable("other!") == None);
    ```
     */
    fn subslice_offset(&self, inner: &Self) -> Option<usize>;
}

impl SubsliceOffset for str {
    fn subslice_offset(&self, inner: &str) -> Option<usize> {
        let self_beg = self.as_ptr() as usize;
        let inner = inner.as_ptr() as usize;
        if inner < self_beg || inner > self_beg.wrapping_add(self.len()) {
            None
        } else {
            Some(inner.wrapping_sub(self_beg))
        }
    }
}
