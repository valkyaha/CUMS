use cums_sekiro::{FsbBank, Codec, Version, AudioSettings, rebuild_ogg, extract_mp3, replace_sample};
use eframe::egui::{self, Color32, RichText, Rounding, Stroke, Vec2};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::io::Cursor;
use std::path::PathBuf;

struct Replacement {
    sound_idx: usize,
    path: PathBuf,
    settings: AudioSettings,
}

struct SoundInfo {
    index: usize,
    name: String,
    duration_secs: f32,
    sample_rate: u32,
    channels: u32,
    modified: bool,
}

struct OpenFile {
    id: usize,
    path: PathBuf,
    bank: FsbBank,
    replacements: Vec<Replacement>,
}

impl OpenFile {
    fn name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".into())
    }

    fn sounds(&self) -> Vec<SoundInfo> {
        self.bank.samples.iter().map(|s| SoundInfo {
            index: s.index,
            name: s.name.clone().unwrap_or_else(|| format!("sound_{}", s.index)),
            duration_secs: if s.frequency > 0 { s.samples as f32 / s.frequency as f32 } else { 0.0 },
            sample_rate: s.frequency,
            channels: s.channels,
            modified: self.replacements.iter().any(|r| r.sound_idx == s.index),
        }).collect()
    }

    fn has_changes(&self) -> bool { !self.replacements.is_empty() }
    fn sample_count(&self) -> usize { self.bank.samples.len() }
}

pub struct CumsApp {
    files: Vec<OpenFile>,
    next_id: usize,
    selected_file: Option<usize>,
    editing_sound: Option<usize>,
    search_query: String,
    file_search_query: String,
    status: String,
    fsbankcl_path: PathBuf,
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    playing: Option<(usize, usize)>,
    playback_volume: f32,
}

impl CumsApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let cwd = std::env::current_dir().unwrap_or_default();
        let pf86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();

        let fsbankcl_path = [
            cwd.join("lib/fmod/fsbankcl.exe"),
            cwd.join("examples/Dark Souls Sound Inserter/fsbankcl.exe"),
            PathBuf::from(&pf86).join("FMOD SoundSystem/FMOD Studio API Universal Windows Platform/bin/fsbankcl.exe"),
        ].into_iter().find(|p| p.exists()).unwrap_or_else(|| cwd.join("fsbankcl.exe"));

        let (stream, handle) = OutputStream::try_default().ok().map(|(s, h)| (Some(s), Some(h))).unwrap_or((None, None));

        Self {
            files: Vec::new(),
            next_id: 0,
            selected_file: None,
            editing_sound: None,
            search_query: String::new(),
            file_search_query: String::new(),
            status: "Ready".into(),
            fsbankcl_path,
            _stream: stream,
            handle,
            sink: None,
            playing: None,
            playback_volume: 0.5,
        }
    }

    fn open_files(&mut self) {
        if let Some(paths) = rfd::FileDialog::new().add_filter("FSB", &["fsb"]).pick_files() {
            for p in paths { self.load_file(p); }
        }
    }

    fn open_folder(&mut self) {
        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
            let mut n = 0;
            if let Ok(entries) = std::fs::read_dir(&folder) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().map(|x| x == "fsb").unwrap_or(false) {
                        self.load_file(p);
                        n += 1;
                    }
                }
            }
            self.status = if n > 0 { format!("Loaded {} files", n) } else { "No FSB files found".into() };
        }
    }

    fn load_file(&mut self, path: PathBuf) {
        if self.files.iter().any(|f| f.path == path) { return; }
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        match FsbBank::load(&path) {
            Ok(bank) => {
                let id = self.next_id;
                self.next_id += 1;
                self.files.push(OpenFile { id, path, bank, replacements: Vec::new() });
                if self.selected_file.is_none() { self.selected_file = Some(id); }
                self.status = format!("Opened {}", name);
            }
            Err(e) => {
                self.status = format!("Failed to load {}: {}", name, e);
            }
        }
    }

    fn close_file(&mut self, id: usize) {
        if self.playing.map(|(f, _)| f) == Some(id) { self.stop(); }
        self.files.retain(|f| f.id != id);
        if self.selected_file == Some(id) {
            self.selected_file = self.files.first().map(|f| f.id);
        }
    }

    fn play(&mut self, file_id: usize, sound_idx: usize) {
        if self.playing == Some((file_id, sound_idx)) { self.stop(); return; }
        self.stop();

        let Some(handle) = &self.handle else { return };
        let Some(file) = self.files.iter().find(|f| f.id == file_id) else { return };
        let sample = &file.bank.samples[sound_idx];

        let audio: Option<Vec<u8>> = match file.bank.codec {
            Codec::Vorbis => rebuild_ogg(&file.bank, sample).ok(),
            Codec::Mpeg => extract_mp3(&file.bank, sample).ok(),
            _ => None,
        };

        if let Some(data) = audio {
            if let Ok(decoder) = Decoder::new(Cursor::new(data)) {
                if let Ok(sink) = Sink::try_new(handle) {
                    sink.set_volume(self.playback_volume);
                    sink.append(decoder);
                    self.sink = Some(sink);
                    self.playing = Some((file_id, sound_idx));
                }
            }
        }
    }

    fn set_playback_volume(&mut self, volume: f32) {
        self.playback_volume = volume.clamp(0.0, 1.0);
        if let Some(sink) = &self.sink { sink.set_volume(self.playback_volume); }
    }

    fn stop(&mut self) {
        if let Some(sink) = self.sink.take() { sink.stop(); }
        self.playing = None;
    }

    fn is_playing(&self) -> bool {
        self.sink.as_ref().map(|s| !s.empty()).unwrap_or(false)
    }

    fn replace(&mut self, file_id: usize, sound_idx: usize) {
        if let Some(path) = rfd::FileDialog::new().add_filter("Audio", &["wav", "mp3", "ogg", "flac"]).pick_file() {
            if let Some(file) = self.files.iter_mut().find(|f| f.id == file_id) {
                file.replacements.retain(|r| r.sound_idx != sound_idx);
                file.replacements.push(Replacement { sound_idx, path: path.clone(), settings: AudioSettings::default() });
                self.editing_sound = Some(sound_idx);
                self.status = format!("Added: {}", path.file_name().unwrap_or_default().to_string_lossy());
            }
        }
    }

    fn extract(&mut self, file_id: usize, sound_idx: usize) {
        let Some(file) = self.files.iter().find(|f| f.id == file_id) else { return };
        let sample = &file.bank.samples[sound_idx];
        let name = sample.name.clone().unwrap_or_else(|| format!("sound_{}", sound_idx));

        let (ext, data): (&str, Option<Vec<u8>>) = match file.bank.codec {
            Codec::Vorbis => ("ogg", rebuild_ogg(&file.bank, sample).ok()),
            Codec::Mpeg => ("mp3", extract_mp3(&file.bank, sample).ok()),
            _ => ("bin", file.bank.sample_data(sound_idx).ok().map(|d| d.to_vec())),
        };

        if let Some(data) = data {
            let fname = format!("{}.{}", name, ext);
            if let Some(path) = rfd::FileDialog::new().set_file_name(&fname).save_file() {
                if std::fs::write(&path, &data).is_ok() {
                    self.status = format!("Exported {}", fname);
                }
            }
        }
    }

    fn extract_all(&mut self, file_id: usize) {
        let Some(file) = self.files.iter().find(|f| f.id == file_id) else { return };
        let Some(folder) = rfd::FileDialog::new().pick_folder() else { return };

        let ext = match file.bank.codec { Codec::Vorbis => "ogg", Codec::Mpeg => "mp3", _ => "bin" };
        let mut count = 0;

        for sample in &file.bank.samples {
            let data: Option<Vec<u8>> = match file.bank.codec {
                Codec::Vorbis => rebuild_ogg(&file.bank, sample).ok(),
                Codec::Mpeg => extract_mp3(&file.bank, sample).ok(),
                _ => file.bank.sample_data(sample.index).ok().map(|d| d.to_vec()),
            };
            if let Some(data) = data {
                let name = sample.name.clone().unwrap_or_else(|| format!("sound_{}", sample.index));
                if std::fs::write(folder.join(format!("{}.{}", name, ext)), &data).is_ok() {
                    count += 1;
                }
            }
        }
        self.status = format!("Exported {} sounds", count);
    }

    fn save(&mut self, file_id: usize) {
        let Some(file) = self.files.iter().find(|f| f.id == file_id) else { return };
        if !file.has_changes() { return; }

        let fname = file.name();
        let Some(out_path) = rfd::FileDialog::new().add_filter("FSB", &["fsb"]).set_file_name(&fname).save_file() else { return };

        let temp = std::env::temp_dir().join("cums");
        let _ = std::fs::create_dir_all(&temp);
        let fmod = self.fsbankcl_path.clone();

        let file = self.files.iter_mut().find(|f| f.id == file_id).unwrap();
        let mods: Vec<_> = file.replacements.iter().map(|r| (r.sound_idx, r.path.clone(), r.settings.clone())).collect();

        let result: Result<(), String> = match file.bank.version {
            Version::Fsb5 => {
                let mut err = None;
                for (idx, path, settings) in &mods {
                    if let Err(e) = replace_sample(&mut file.bank, *idx, path, &fmod, &temp, settings) {
                        err = Some(e.to_string());
                        break;
                    }
                }
                err.map(Err).unwrap_or_else(|| {
                    file.bank.save(&out_path, file.bank.encryption != cums_sekiro::Encryption::None)
                        .map_err(|e| e.to_string())
                })
            }
            Version::Fsb4 => {
                let mut err = None;
                for (idx, path, _) in &mods {
                    if let Err(e) = file.bank.replace_sample(*idx, path, &temp) {
                        err = Some(e.to_string());
                        break;
                    }
                }
                err.map(Err).unwrap_or_else(|| file.bank.save(&out_path, false).map_err(|e| e.to_string()))
            }
        };

        match result {
            Ok(_) => {
                file.replacements.clear();
                self.editing_sound = None;
                self.status = format!("Saved to {}", out_path.file_name().unwrap_or_default().to_string_lossy());
            }
            Err(e) => self.status = format!("Error: {}", e),
        }
    }
}

impl eframe::App for CumsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut style = (*ctx.style()).clone();
        style.visuals.window_rounding = Rounding::same(12.0);
        style.visuals.widgets.noninteractive.rounding = Rounding::same(8.0);
        style.visuals.widgets.inactive.rounding = Rounding::same(8.0);
        style.visuals.widgets.hovered.rounding = Rounding::same(8.0);
        style.visuals.widgets.active.rounding = Rounding::same(8.0);
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(16.0, 8.0);
        ctx.set_style(style);

        let bg_dark = Color32::from_rgb(15, 15, 20);
        let bg_panel = Color32::from_rgb(25, 25, 32);
        let bg_card = Color32::from_rgb(35, 35, 45);
        let bg_hover = Color32::from_rgb(45, 45, 58);
        let accent = Color32::from_rgb(99, 102, 241);
        let accent_dim = Color32::from_rgb(79, 82, 201);
        let success = Color32::from_rgb(34, 197, 94);
        let warning = Color32::from_rgb(251, 191, 36);
        let text = Color32::from_rgb(248, 250, 252);
        let text_dim = Color32::from_rgb(148, 163, 184);

        if self.playing.is_some() && !self.is_playing() { self.playing = None; }

        ctx.input(|i| {
            for f in &i.raw.dropped_files {
                if let Some(p) = &f.path {
                    if p.is_dir() {
                        if let Ok(entries) = std::fs::read_dir(p) {
                            for e in entries.flatten() {
                                let path = e.path();
                                if path.extension().map(|x| x == "fsb").unwrap_or(false) {
                                    self.load_file(path);
                                }
                            }
                        }
                    } else {
                        self.load_file(p.clone());
                    }
                }
            }
        });

        egui::SidePanel::left("sidebar")
            .exact_width(280.0)
            .frame(egui::Frame::none().fill(bg_panel).inner_margin(16.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| { ui.heading(RichText::new("CUMS").size(28.0).color(accent).strong()); });
                ui.label(RichText::new("Audio Modding Tool").size(12.0).color(text_dim));
                ui.add_space(24.0);

                if ui.add_sized([ui.available_width(), 36.0], egui::Button::new(RichText::new("Open Files").color(text)).fill(bg_card)).clicked() {
                    self.open_files();
                }
                ui.add_space(4.0);
                if ui.add_sized([ui.available_width(), 36.0], egui::Button::new(RichText::new("Open Folder").color(text)).fill(bg_card)).clicked() {
                    self.open_folder();
                }

                ui.add_space(24.0);
                ui.separator();
                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("OPEN FILES").size(11.0).color(text_dim).strong());
                    if !self.files.is_empty() {
                        ui.label(RichText::new(format!("({})", self.files.len())).size(11.0).color(text_dim));
                    }
                });
                ui.add_space(8.0);

                if !self.files.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.file_search_query).hint_text("Filter...").desired_width(ui.available_width() - 8.0));
                    });
                    ui.add_space(8.0);
                }

                let mut close_id = None;
                let mut select_id = None;
                let bottom_height = 120.0;
                let available = (ui.available_height() - bottom_height).max(100.0);
                let file_query = self.file_search_query.to_lowercase();

                egui::ScrollArea::vertical().max_height(available).auto_shrink([false, false]).show(ui, |ui| {
                    for file in self.files.iter().filter(|f| file_query.is_empty() || f.name().to_lowercase().contains(&file_query)) {
                        let is_selected = self.selected_file == Some(file.id);
                        let bg = if is_selected { accent_dim } else { bg_card };

                        let resp = egui::Frame::none().fill(bg).rounding(8.0).inner_margin(12.0).outer_margin(egui::Margin::symmetric(0.0, 2.0)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    let name = if file.has_changes() { format!("{} *", file.name()) } else { file.name() };
                                    ui.label(RichText::new(name).color(text).strong());
                                    ui.label(RichText::new(format!("{} sounds", file.sample_count())).size(11.0).color(text_dim));
                                });
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("X").clicked() { close_id = Some(file.id); }
                                });
                            });
                        });
                        if resp.response.interact(egui::Sense::click()).clicked() { select_id = Some(file.id); }
                    }
                });

                if let Some(id) = close_id { self.close_file(id); }
                if let Some(id) = select_id { self.selected_file = Some(id); self.editing_sound = None; }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    ui.label(RichText::new(&self.status).size(11.0).color(text_dim));
                    ui.add_space(12.0);

                    egui::Frame::none().fill(bg_card).rounding(8.0).inner_margin(12.0).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let icon = if self.playback_volume < 0.01 { "M" } else if self.playback_volume < 0.5 { "L" } else { "H" };
                            ui.label(RichText::new(icon).size(14.0));
                            let mut vol = self.playback_volume;
                            if ui.add(egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false)).changed() {
                                self.set_playback_volume(vol);
                            }
                            ui.label(RichText::new(format!("{:.0}%", self.playback_volume * 100.0)).size(11.0).color(text_dim).monospace());
                        });
                    });
                    ui.add_space(8.0);
                    ui.label(RichText::new("VOLUME").size(10.0).color(text_dim));
                });
            });

        egui::CentralPanel::default().frame(egui::Frame::none().fill(bg_dark).inner_margin(24.0)).show(ctx, |ui| {
            if self.files.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(RichText::new("Drop FSB files here").size(24.0).color(text));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Supports Sekiro, Dark Souls 1/2/3").size(12.0).color(text_dim));
                    });
                });
                return;
            }

            let Some(file_id) = self.selected_file else { return };
            let (has_changes, sounds, replacements, file_name) = {
                let Some(file) = self.files.iter().find(|f| f.id == file_id) else { return };
                let repl: Vec<(usize, f32, f32, f32)> = file.replacements.iter()
                    .map(|r| (r.sound_idx, r.settings.volume_db, r.settings.pitch_semitones, r.settings.speed)).collect();
                (file.has_changes(), file.sounds(), repl, file.name())
            };

            let playing = self.playing;
            let is_playing = self.is_playing();
            let editing_sound = self.editing_sound;

            let mut do_extract_all = false;
            let mut do_save = false;

            ui.horizontal(|ui| {
                ui.label(RichText::new(&file_name).size(20.0).color(text).strong());
                ui.label(RichText::new(format!("({} sounds)", sounds.len())).color(text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if has_changes {
                        if ui.add(egui::Button::new(RichText::new("Save").color(Color32::WHITE)).fill(accent)).clicked() {
                            do_save = true;
                        }
                    }
                    if ui.button("Export All").clicked() { do_extract_all = true; }
                });
            });

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.search_query).hint_text("Search sounds...").desired_width(300.0));
            });
            ui.add_space(16.0);

            let mut action: Option<(usize, &str)> = None;
            let mut settings_change: Option<(usize, f32, f32, f32)> = None;

            egui::ScrollArea::vertical().show(ui, |ui| {
                let query = self.search_query.to_lowercase();
                for sound in sounds.iter().filter(|s| query.is_empty() || s.name.to_lowercase().contains(&query)) {
                    let is_playing_this = playing == Some((file_id, sound.index)) && is_playing;
                    let is_editing = editing_sound == Some(sound.index);
                    let card_bg = if is_playing_this { bg_hover } else { bg_card };

                    egui::Frame::none().fill(card_bg).rounding(12.0)
                        .stroke(if sound.modified { Stroke::new(1.0, warning) } else { Stroke::NONE })
                        .inner_margin(16.0).outer_margin(egui::Margin::symmetric(0.0, 4.0)).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let play_icon = if is_playing_this { "Stop" } else { "Play" };
                                let play_color = if is_playing_this { success } else { accent };
                                if ui.add(egui::Button::new(RichText::new(play_icon).color(play_color)).min_size(Vec2::new(60.0, 36.0)).fill(bg_dark)).clicked() {
                                    action = Some((sound.index, "play"));
                                }

                                ui.add_space(12.0);
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&sound.name).color(text).strong());
                                        if sound.modified { ui.label(RichText::new("Modified").size(10.0).color(warning)); }
                                    });
                                    let mins = sound.duration_secs as u32 / 60;
                                    let secs = sound.duration_secs as u32 % 60;
                                    let ch = if sound.channels == 1 { "Mono" } else { "Stereo" };
                                    ui.label(RichText::new(format!("{}:{:02} | {}Hz | {}", mins, secs, sound.sample_rate, ch)).size(12.0).color(text_dim));
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("Export").clicked() { action = Some((sound.index, "extract")); }
                                    if ui.add(egui::Button::new("Replace").fill(accent_dim)).clicked() { action = Some((sound.index, "replace")); }
                                    if sound.modified {
                                        if ui.button(if is_editing { "- Settings" } else { "+ Settings" }).clicked() {
                                            action = Some((sound.index, "toggle_settings"));
                                        }
                                    }
                                });
                            });

                            if sound.modified && is_editing {
                                ui.add_space(12.0);
                                egui::Frame::none().fill(Color32::from_rgb(20, 20, 28)).rounding(12.0).inner_margin(20.0).show(ui, |ui| {
                                    if let Some(repl) = replacements.iter().find(|r| r.0 == sound.index) {
                                        let (_, vol, pitch, spd) = *repl;
                                        let mut new_vol = vol;
                                        let mut new_pitch = pitch;
                                        let mut new_speed = spd;

                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new("Audio Settings").color(text).size(14.0).strong());
                                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                if ui.button("Reset").clicked() {
                                                    new_vol = 0.0; new_pitch = 0.0; new_speed = 1.0;
                                                }
                                            });
                                        });
                                        ui.add_space(12.0);

                                        egui::Grid::new("audio_controls").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                                            ui.label(RichText::new("Volume").color(text).size(12.0));
                                            ui.add(egui::Slider::new(&mut new_vol, -20.0..=20.0).suffix(" dB").step_by(0.5));
                                            ui.end_row();

                                            ui.label(RichText::new("Pitch").color(text).size(12.0));
                                            ui.add(egui::Slider::new(&mut new_pitch, -12.0..=12.0).suffix(" st").step_by(0.5));
                                            ui.end_row();

                                            ui.label(RichText::new("Speed").color(text).size(12.0));
                                            ui.add(egui::Slider::new(&mut new_speed, 0.5..=2.0).step_by(0.05));
                                            ui.end_row();
                                        });

                                        if (new_vol - vol).abs() > 0.01 || (new_pitch - pitch).abs() > 0.01 || (new_speed - spd).abs() > 0.01 {
                                            settings_change = Some((sound.index, new_vol, new_pitch, new_speed));
                                        }
                                    }
                                });
                            }
                        });
                }
            });

            if let Some((idx, act)) = action {
                match act {
                    "play" => self.play(file_id, idx),
                    "replace" => self.replace(file_id, idx),
                    "extract" => self.extract(file_id, idx),
                    "toggle_settings" => {
                        if self.editing_sound == Some(idx) { self.editing_sound = None; }
                        else { self.editing_sound = Some(idx); }
                    }
                    _ => {}
                }
            }

            if let Some((idx, vol, pitch, speed)) = settings_change {
                if let Some(file) = self.files.iter_mut().find(|f| f.id == file_id) {
                    if let Some(repl) = file.replacements.iter_mut().find(|r| r.sound_idx == idx) {
                        repl.settings.volume_db = vol;
                        repl.settings.pitch_semitones = pitch;
                        repl.settings.speed = speed;
                    }
                }
            }

            if do_extract_all { self.extract_all(file_id); }
            if do_save { self.save(file_id); }
        });

        if self.is_playing() { ctx.request_repaint(); }
    }
}
