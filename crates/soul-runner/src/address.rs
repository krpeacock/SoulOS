//! Address book — a vCard-based contact manager.
//!
//! Stores contacts as vCard 3.0 records in the database, with a simple
//! list interface for browsing and editing. Each contact is a complete
//! vCard that can be exported/imported.

use embedded_graphics::{
    draw_target::DrawTarget, pixelcolor::Gray8, prelude::*, primitives::Rectangle,
};
use soul_core::{App, Ctx, Event, KeyCode, HardButton, SCREEN_WIDTH};
use soul_db::Database;
use soul_ui::{
    title_bar, button, label, hit_test, TextInput, Pagination, PaginationAction, TITLE_BAR_H,
};
use std::fs;
use std::path::PathBuf;
use std::str;

/// Basic vCard data structure for SoulOS contacts
#[derive(Clone, Debug)]
pub struct VCard {
    pub fn_name: String,     // Full name (FN field)
    pub family: String,      // Family name (N field part)
    pub given: String,       // Given name (N field part)
    pub phone: String,       // Primary phone (TEL field)
    pub email: String,       // Primary email (EMAIL field)
    pub note: String,        // Notes (NOTE field)
}

impl VCard {
    fn new() -> Self {
        Self {
            fn_name: String::new(),
            family: String::new(), 
            given: String::new(),
            phone: String::new(),
            email: String::new(),
            note: String::new(),
        }
    }

    fn from_vcard_data(data: &[u8]) -> Result<Self, &'static str> {
        let content = str::from_utf8(data).map_err(|_| "Invalid UTF-8")?;
        let mut card = VCard::new();
        
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line == "BEGIN:VCARD" || line == "END:VCARD" {
                continue;
            }
            
            if let Some((key, value)) = line.split_once(':') {
                match key.trim() {
                    "FN" => card.fn_name = value.trim().to_string(),
                    "N" => {
                        // N field format: Family;Given;Middle;Prefix;Suffix
                        let parts: Vec<&str> = value.split(';').collect();
                        if !parts.is_empty() {
                            card.family = parts[0].trim().to_string();
                        }
                        if parts.len() > 1 {
                            card.given = parts[1].trim().to_string();
                        }
                    }
                    key if key.starts_with("TEL") => {
                        if card.phone.is_empty() {
                            card.phone = value.trim().to_string();
                        }
                    }
                    key if key.starts_with("EMAIL") => {
                        if card.email.is_empty() {
                            card.email = value.trim().to_string();
                        }
                    }
                    "NOTE" => card.note = value.trim().to_string(),
                    _ => {} // Ignore other fields for now
                }
            }
        }
        
        // If FN is empty, construct it from given/family
        if card.fn_name.is_empty() && (!card.given.is_empty() || !card.family.is_empty()) {
            card.fn_name = format!("{} {}", card.given, card.family).trim().to_string();
        }
        
        Ok(card)
    }

    fn to_vcard_data(&self) -> Vec<u8> {
        let mut vcard = String::new();
        vcard.push_str("BEGIN:VCARD\n");
        vcard.push_str("VERSION:3.0\n");
        
        if !self.fn_name.is_empty() {
            vcard.push_str(&format!("FN:{}\n", self.fn_name));
        }
        
        if !self.family.is_empty() || !self.given.is_empty() {
            vcard.push_str(&format!("N:{};{};;;\n", self.family, self.given));
        }
        
        if !self.phone.is_empty() {
            vcard.push_str(&format!("TEL;TYPE=VOICE:{}\n", self.phone));
        }
        
        if !self.email.is_empty() {
            vcard.push_str(&format!("EMAIL:{}\n", self.email));
        }
        
        if !self.note.is_empty() {
            vcard.push_str(&format!("NOTE:{}\n", self.note));
        }
        
        vcard.push_str("END:VCARD\n");
        vcard.into_bytes()
    }

    fn display_name(&self) -> String {
        if !self.fn_name.is_empty() {
            self.fn_name.clone()
        } else if !self.given.is_empty() || !self.family.is_empty() {
            format!("{} {}", self.given, self.family).trim().to_string()
        } else {
            "Unnamed Contact".to_string()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    List,
    View(u32),   // Contact ID
    Edit(u32),   // Contact ID, 0 for new contact
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditField {
    FullName,
    GivenName,
    FamilyName,
    Phone,
    Email,
    Note,
}

const EDIT_FIELDS: &[EditField] = &[
    EditField::FullName,
    EditField::GivenName, 
    EditField::FamilyName,
    EditField::Phone,
    EditField::Email,
    EditField::Note,
];

pub struct Address {
    db: Database,
    db_path: PathBuf,
    mode: Mode,
    contacts: Vec<(u32, String)>, // ID and display name pairs
    pagination: Pagination,
    edit_card: VCard,
    edit_inputs: [TextInput; 6], // One for each field
    active_field: usize,
    edit_id: u32, // Contact ID being edited, 0 for new
}

impl Address {
    pub fn new() -> Self {
        let db_path = Self::contacts_db_path();
        let mut db = Self::load_or_create_db(&db_path);
        
        // Create a sample contact if database is empty
        if db.iter_category(0).next().is_none() {
            let sample = VCard {
                fn_name: "John Doe".to_string(),
                family: "Doe".to_string(),
                given: "John".to_string(),
                phone: "+1-555-0123".to_string(),
                email: "john.doe@example.com".to_string(),
                note: "Sample contact".to_string(),
            };
            db.insert(0, sample.to_vcard_data());
        }
        
        let pagination_area = Rectangle::new(Point::new(16, 250), Size::new(208, 30));
        let edit_inputs = [
            TextInput::new(Self::field_input_rect(0)), // Full Name
            TextInput::new(Self::field_input_rect(1)), // Given Name
            TextInput::new(Self::field_input_rect(2)), // Family Name
            TextInput::new(Self::field_input_rect(3)), // Phone
            TextInput::new(Self::field_input_rect(4)), // Email
            TextInput::new(Self::field_input_rect(5)), // Note
        ];
        
        let mut addr = Self {
            db,
            db_path,
            mode: Mode::List,
            contacts: Vec::new(),
            pagination: Pagination::new(pagination_area, 8),
            edit_card: VCard::new(),
            edit_inputs,
            active_field: 0,
            edit_id: 0,
        };
        
        addr.refresh_contact_list();
        addr
    }

    fn contacts_db_path() -> PathBuf {
        std::env::var("SOUL_CONTACTS_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(".soulos/contacts.sdb"))
    }

    fn load_or_create_db(path: &PathBuf) -> Database {
        if let Ok(bytes) = fs::read(path) {
            if let Some(db) = Database::decode(&bytes) {
                if Self::contacts_db_valid(&db) {
                    return db;
                }
            }
        }
        Database::new("contacts")
    }

    fn contacts_db_valid(db: &Database) -> bool {
        let mut expected = [0u8; 32];
        for (i, b) in b"contacts".iter().enumerate() {
            expected[i] = *b;
        }
        db.name == expected
    }

    fn persist(&self) {
        if let Some(parent) = self.db_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("contacts: could not create cache directory: {e}");
                return;
            }
        }
        if let Err(e) = fs::write(&self.db_path, self.db.encode()) {
            eprintln!("contacts: could not persist to {}: {e}", self.db_path.display());
        }
    }

    fn refresh_contact_list(&mut self) {
        self.contacts.clear();
        
        for record in self.db.iter_category(0) {
            if let Ok(vcard) = VCard::from_vcard_data(&record.data) {
                let display = vcard.display_name();
                self.contacts.push((record.id, display));
            }
        }
        
        // Sort by display name
        self.contacts.sort_by(|a, b| a.1.cmp(&b.1));
        
        // Update pagination
        self.pagination.set_total_items(self.contacts.len());
    }

    fn field_input_rect(field_index: usize) -> Rectangle {
        let y = 50 + (field_index as i32 * 35); // 30 for field + 5 padding
        Rectangle::new(Point::new(100, y), Size::new(130, 18))
    }

    fn field_label_point(field_index: usize) -> Point {
        let y = 50 + (field_index as i32 * 35) + 4; // Align with input
        Point::new(12, y)
    }

    fn list_rect() -> Rectangle {
        Rectangle::new(
            Point::new(8, TITLE_BAR_H as i32 + 40),
            Size::new(224, 200),
        )
    }

    fn save_current_edit(&mut self) {
        // Update VCard with all field values
        self.edit_card.fn_name = self.edit_inputs[0].text().to_string();
        self.edit_card.given = self.edit_inputs[1].text().to_string();
        self.edit_card.family = self.edit_inputs[2].text().to_string();
        self.edit_card.phone = self.edit_inputs[3].text().to_string();
        self.edit_card.email = self.edit_inputs[4].text().to_string();
        self.edit_card.note = self.edit_inputs[5].text().to_string();
        
        // Save to database
        let vcard_data = self.edit_card.to_vcard_data();
        if self.edit_id == 0 {
            // New contact
            self.db.insert(0, vcard_data);
        } else {
            // Update existing contact
            self.db.update(self.edit_id, vcard_data);
        }
        
        self.refresh_contact_list();
        self.persist();
    }

    fn load_contact_for_edit(&mut self, contact_id: u32) {
        if let Some(record) = self.db.get(contact_id) {
            if let Ok(vcard) = VCard::from_vcard_data(&record.data) {
                self.edit_card = vcard.clone();
                self.edit_id = contact_id;
                self.active_field = 0;
                self.update_all_edit_inputs();
            }
        }
    }

    fn start_new_contact(&mut self) {
        self.edit_card = VCard::new();
        self.edit_id = 0;
        self.active_field = 0;
        self.update_all_edit_inputs();
    }

    fn update_all_edit_inputs(&mut self) {
        let values = [
            &self.edit_card.fn_name,
            &self.edit_card.given,
            &self.edit_card.family,
            &self.edit_card.phone,
            &self.edit_card.email,
            &self.edit_card.note,
        ];
        
        for (i, value) in values.iter().enumerate() {
            self.edit_inputs[i] = TextInput::new(Self::field_input_rect(i));
            let _ = self.edit_inputs[i].set_text(value.to_string());
        }
    }

    fn next_field(&mut self) {
        self.active_field = (self.active_field + 1) % EDIT_FIELDS.len();
    }

    fn prev_field(&mut self) {
        self.active_field = if self.active_field == 0 {
            EDIT_FIELDS.len() - 1
        } else {
            self.active_field - 1
        };
    }

}

impl App for Address {
    fn handle(&mut self, event: Event, ctx: &mut Ctx<'_>) {
        match event {
            Event::Menu => {
                // Menu toggles back to list or creates new contact
                match self.mode {
                    Mode::List => {
                        // Start editing new contact
                        self.start_new_contact();
                        self.mode = Mode::Edit(0);
                        ctx.invalidate_all();
                    }
                    Mode::View(contact_id) => {
                        // Edit existing contact
                        self.load_contact_for_edit(contact_id);
                        self.mode = Mode::Edit(contact_id);
                        ctx.invalidate_all();
                    }
                    Mode::Edit(_) => {
                        // Save and return to list
                        self.save_current_edit();
                        self.mode = Mode::List;
                        ctx.invalidate_all();
                    }
                }
            }
            Event::Key(KeyCode::Char(c)) => {
                if let Mode::Edit(_) = self.mode {
                    let out = self.edit_inputs[self.active_field].insert_char(c);
                    if let Some(r) = out.dirty {
                        ctx.invalidate(r);
                    }
                }
            }
            Event::Key(KeyCode::Backspace) => {
                if let Mode::Edit(_) = self.mode {
                    let out = self.edit_inputs[self.active_field].backspace();
                    if let Some(r) = out.dirty {
                        ctx.invalidate(r);
                    }
                }
            }
            Event::Key(KeyCode::Enter) => {
                if let Mode::Edit(_) = self.mode {
                    // Enter moves to next field
                    self.next_field();
                    ctx.invalidate_all();
                }
            }
            Event::Key(KeyCode::ArrowUp) => {
                if let Mode::Edit(_) = self.mode {
                    self.prev_field();
                    ctx.invalidate_all();
                }
            }
            Event::Key(KeyCode::ArrowDown) => {
                if let Mode::Edit(_) = self.mode {
                    self.next_field();
                    ctx.invalidate_all();
                }
            }
            Event::ButtonDown(HardButton::PageUp) => {
                if let Mode::List = self.mode {
                    if self.pagination.prev_page() {
                        ctx.invalidate_all();
                    }
                }
            }
            Event::ButtonDown(HardButton::PageDown) => {
                if let Mode::List = self.mode {
                    if self.pagination.next_page() {
                        ctx.invalidate_all();
                    }
                }
            }
            Event::PenUp { x, y } => {
                match self.mode {
                    Mode::List => {
                        // Handle pagination taps
                        if let Some(action) = self.pagination.handle_pen(x, y) {
                            match action {
                                PaginationAction::PrevPage => {
                                    if self.pagination.prev_page() {
                                        ctx.invalidate_all();
                                    }
                                }
                                PaginationAction::NextPage => {
                                    if self.pagination.next_page() {
                                        ctx.invalidate_all();
                                    }
                                }
                            }
                            return;
                        }

                        // Handle contact list taps
                        let list_rect = Self::list_rect();
                        if hit_test(&list_rect, x, y) {
                            let rel_y = y as i32 - list_rect.top_left.y;
                            let item_h = 24;
                            let idx = (rel_y / item_h) as usize + self.pagination.page_start_index();
                            
                            if let Some((contact_id, _)) = self.contacts.get(idx) {
                                self.mode = Mode::View(*contact_id);
                                ctx.invalidate_all();
                            }
                        }
                    }
                    Mode::View(_contact_id) => {
                        // Tap anywhere to go back to list
                        self.mode = Mode::List;
                        ctx.invalidate_all();
                    }
                    Mode::Edit(_) => {
                        // Handle edit mode taps (field selection)
                        for (i, input) in self.edit_inputs.iter_mut().enumerate() {
                            if input.contains(x, y) {
                                self.active_field = i;
                                let _ = input.pen_released(x, y);
                                ctx.invalidate_all();
                                return;
                            }
                        }
                    }
                }
            }
            Event::AppStop => {
                self.persist();
            }
            _ => {}
        }
    }

    fn draw<D>(&mut self, canvas: &mut D)
    where
        D: DrawTarget<Color = Gray8>,
    {
        let _ = title_bar(canvas, SCREEN_WIDTH as u32, "Address");

        match self.mode {
            Mode::List => {
                let _ = label(canvas, Point::new(12, TITLE_BAR_H as i32 + 24), "Contacts");
                
                if self.contacts.is_empty() {
                    let _ = label(canvas, Point::new(16, 80), "No contacts. Press Menu to add.");
                } else {
                    let list_rect = Self::list_rect();
                    let item_h = 24;
                    
                    let page_start = self.pagination.page_start_index();
                    let page_end = self.pagination.page_end_index();
                    
                    for (i, (_, name)) in self.contacts[page_start..page_end]
                        .iter()
                        .enumerate()
                    {
                        let y = list_rect.top_left.y + (i as i32 * item_h);
                        let _ = button(
                            canvas,
                            Rectangle::new(Point::new(16, y), Size::new(208, 20)),
                            name,
                            false,
                        );
                    }

                    // Draw pagination widget
                    self.pagination.draw(canvas);
                }
                
                let _ = label(canvas, Point::new(12, 270), "Menu: New Contact");
            }
            Mode::View(contact_id) => {
                if let Some(record) = self.db.get(contact_id) {
                    if let Ok(vcard) = VCard::from_vcard_data(&record.data) {
                        let mut y = 40;
                        
                        if !vcard.fn_name.is_empty() {
                            let _ = label(canvas, Point::new(12, y), &vcard.fn_name);
                            y += 20;
                        }
                        
                        if !vcard.given.is_empty() || !vcard.family.is_empty() {
                            let name = format!("{} {}", vcard.given, vcard.family).trim().to_string();
                            if name != vcard.fn_name {
                                let _ = label(canvas, Point::new(12, y), &format!("Name: {}", name));
                                y += 20;
                            }
                        }
                        
                        if !vcard.phone.is_empty() {
                            let _ = label(canvas, Point::new(12, y), &format!("Phone: {}", vcard.phone));
                            y += 20;
                        }
                        
                        if !vcard.email.is_empty() {
                            let _ = label(canvas, Point::new(12, y), &format!("Email: {}", vcard.email));
                            y += 20;
                        }
                        
                        if !vcard.note.is_empty() {
                            let _ = label(canvas, Point::new(12, y), &format!("Note: {}", vcard.note));
                        }
                    }
                }
                let _ = label(canvas, Point::new(12, 250), "Menu: Edit");
                let _ = label(canvas, Point::new(12, 270), "Tap: Back to List");
            }
            Mode::Edit(_) => {
                let _ = label(canvas, Point::new(12, 40), "Edit Contact");
                
                let field_names = [
                    "Full Name:",
                    "Given Name:",
                    "Family Name:",
                    "Phone:",
                    "Email:", 
                    "Note:",
                ];
                
                // Draw all fields
                for (i, name) in field_names.iter().enumerate() {
                    let label_point = Self::field_label_point(i);
                    
                    // Highlight active field label
                    if i == self.active_field {
                        let _ = label(canvas, Point::new(label_point.x - 2, label_point.y), ">");
                    }
                    
                    let _ = label(canvas, label_point, name);
                    let _ = self.edit_inputs[i].draw(canvas);
                }
                
                let _ = label(canvas, Point::new(12, 260), "Enter/↑↓: Navigate");
                let _ = label(canvas, Point::new(12, 280), "Menu: Save & Back");
            }
        }
    }
}