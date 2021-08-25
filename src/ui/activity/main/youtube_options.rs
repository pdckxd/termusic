/**
 * MIT License
 *
 * termusic - Copyright (c) 2021 Larry Hao
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
use super::{MainActivity, COMPONENT_TREEVIEW};
use crate::invidious::{InvidiousInstance, YoutubeVideo};
use crate::ui::components::table;
use anyhow::{anyhow, bail, Result};
use humantime::format_duration;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use tuirealm::props::{TableBuilder, TextSpan};
use tuirealm::{Payload, PropsBuilder, Value};
use unicode_truncate::{Alignment, UnicodeTruncateStr};
use ytd_rs::{Arg, ResultType, YoutubeDL};

lazy_static! {
    static ref RE_FILENAME: Regex =
        Regex::new(r"\[ffmpeg\] Destination: (?P<name>.*)\.mp3").unwrap();
}

pub struct YoutubeOptions {
    items: Vec<YoutubeVideo>,
    page: u32,
    search_word: String,
    invidious_instance: InvidiousInstance,
}

impl YoutubeOptions {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            page: 1,
            search_word: "".to_string(),
            invidious_instance: crate::invidious::InvidiousInstance::default(),
        }
    }
    pub fn get_by_index(&self, index: usize) -> Result<&YoutubeVideo> {
        if let Some(item) = self.items.get(index) {
            return Ok(item);
        }
        Err(anyhow!("index not found"))
    }

    pub fn search(&mut self, keyword: &str) -> Result<()> {
        self.search_word = keyword.to_string();
        match crate::invidious::InvidiousInstance::new(keyword) {
            Ok((instance, result)) => {
                self.invidious_instance = instance;
                self.items = result;
                self.page = 1;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
    pub fn prev_page(&mut self) -> Result<()> {
        if self.page > 1 {
            self.page -= 1;
            match self
                .invidious_instance
                .get_search_query(self.search_word.as_str(), self.page)
            {
                Ok(y) => {
                    self.items = y;
                    Ok(())
                }
                Err(e) => Err(e),
            }
        } else {
            Ok(())
        }
    }
    pub fn next_page(&mut self) -> Result<()> {
        self.page += 1;
        match self
            .invidious_instance
            .get_search_query(self.search_word.as_str(), self.page)
        {
            Ok(y) => {
                self.items = y;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn page(&self) -> u32 {
        self.page
    }
}

impl MainActivity {
    pub fn youtube_options_download(&mut self, index: usize) -> Result<()> {
        // download from search result here
        let mut url = "https://www.youtube.com/watch?v=".to_string();
        if let Ok(item) = self.youtube_options.get_by_index(index) {
            url.push_str(&item.video_id);
            if self.youtube_dl(url.as_ref()).is_err() {
                bail!("Error download");
            }
        }
        Ok(())
    }

    pub fn youtube_options_search(&mut self, keyword: &str) {
        match self.youtube_options.search(keyword) {
            Ok(()) => self.sync_youtube_options(),
            Err(e) => self.mount_error(format!("search error: {}", e).as_str()),
        }
    }

    pub fn youtube_options_prev_page(&mut self) {
        match self.youtube_options.prev_page() {
            Ok(_) => self.sync_youtube_options(),
            Err(e) => self.mount_error(format!("search error: {}", e).as_str()),
        }
    }
    pub fn youtube_options_next_page(&mut self) {
        match self.youtube_options.next_page() {
            Ok(_) => self.sync_youtube_options(),
            Err(e) => self.mount_error(format!("search error: {}", e).as_str()),
        }
    }
    pub fn sync_youtube_options(&mut self) {
        if self.youtube_options.items.is_empty() {
            if let Some(props) = self.view.get_props(super::COMPONENT_SCROLLTABLE_YOUTUBE) {
                if let Some(domain) = &self.youtube_options.invidious_instance.domain {
                    let props = table::TablePropsBuilder::from(props)
                        .with_table(
                            TableBuilder::default()
                                .add_col(TextSpan::from(format!(
                                    "Empty result.Probably {} is down.",
                                    domain
                                )))
                                .build(),
                        )
                        .build();
                    let msg = self
                        .view
                        .update(super::COMPONENT_SCROLLTABLE_YOUTUBE, props);
                    self.update(msg);
                }
            }
            return;
        }

        let mut table: TableBuilder = TableBuilder::default();
        for (idx, record) in self.youtube_options.items.iter().enumerate() {
            if idx > 0 {
                table.add_row();
            }
            let duration = record.length_seconds;
            let duration_string = format!("{}", format_duration(Duration::from_secs(duration)));
            let duration_truncated = duration_string.unicode_pad(6, Alignment::Left, true);

            let title = record.title.as_str();

            table
                .add_col(TextSpan::new(
                    format!("[{}] ", duration_truncated,).as_str(),
                ))
                .add_col(TextSpan::new(title).bold());
        }
        let table = table.build();

        if let Some(props) = self.view.get_props(super::COMPONENT_SCROLLTABLE_YOUTUBE) {
            if let Some(domain) = &self.youtube_options.invidious_instance.domain {
                let title = format!(
                    "── Page {} ──┼─ {} ─┼─ {} ─────",
                    self.youtube_options.page(),
                    "Tab/Shift+Tab switch pages",
                    domain,
                );
                let props = table::TablePropsBuilder::from(props)
                    .with_title(title, tuirealm::tui::layout::Alignment::Left)
                    .with_header(&["Duration", "Name"])
                    .with_widths(&[15, 85])
                    .with_table(table)
                    .build();
                self.view
                    .update(super::COMPONENT_SCROLLTABLE_YOUTUBE, props);
            }
        }
    }

    pub fn youtube_dl(&mut self, link: &str) -> Result<()> {
        let mut path: PathBuf = PathBuf::new();
        if let Some(Payload::One(Value::Str(node_id))) = self.view.get_state(COMPONENT_TREEVIEW) {
            let p: &Path = Path::new(node_id.as_str());
            if p.is_dir() {
                path = PathBuf::from(p);
            } else if let Some(p) = p.parent() {
                path = p.to_path_buf();
            }
        }

        let args = vec![
            // Arg::new("--quiet"),
            Arg::new("--extract-audio"),
            Arg::new_with_arg("--audio-format", "mp3"),
            Arg::new("--add-metadata"),
            Arg::new("--embed-thumbnail"),
            Arg::new_with_arg("--metadata-from-title", "%(artist) - %(title)s"),
            Arg::new("--write-sub"),
            Arg::new("--all-subs"),
            Arg::new_with_arg("--convert-subs", "lrc"),
            Arg::new_with_arg("--output", "%(title).90s.%(ext)s"),
        ];
        let ytd = YoutubeDL::new(&path, args, link)?;
        let tx = self.sender.clone();

        thread::spawn(move || {
            let _ = tx.send(super::TransferState::Running);
            // start download
            let download = ytd.download();

            // check what the result is and print out the path to the download or the error
            match download.result_type() {
                ResultType::SUCCESS => {
                    // here we extract the full file name from download output
                    match extract_filepath(download.output(), &path.to_string_lossy()) {
                        Ok(file_fullname) => {
                            let id3_tag = match id3::Tag::read_from_path(&file_fullname) {
                                Ok(tag) => tag,
                                Err(_) => {
                                    let mut t = id3::Tag::new();
                                    let p: &Path = Path::new(&file_fullname);
                                    if let Some(p_base) = p.file_stem() {
                                        t.set_title(p_base.to_string_lossy());
                                    }
                                    let _ = t.write_to_path(p, id3::Version::Id3v24);
                                    t
                                }
                            };
                            // pathToFile, _ := filepath.Split(audioPath)
                            // files, err := ioutil.ReadDir(pathToFile)
                            // var lyricWritten int = 0
                            // for _, file := range files {
                            // 	fileName := file.Name()
                            // 	fileExt := filepath.Ext(fileName)
                            // 	lyricFileName := filepath.Join(pathToFile, fileName)
                            // 	if fileExt == ".lrc" {
                            // 		// Embed all lyrics and use langExt as content descriptor of uslt
                            // 		fileNameWithoutExt := strings.TrimSuffix(fileName, fileExt)
                            // 		langExt := strings.TrimPrefix(filepath.Ext(fileNameWithoutExt), ".")

                            // 		// Read entire file content, giving us little control but
                            // 		// making it very simple. No need to close the file.
                            // 		byteContent, err := ioutil.ReadFile(lyricFileName)
                            // 		lyricContent := string(byteContent)

                            // 		var lyric lyric.Lyric
                            // 		err = lyric.NewFromLRC(lyricContent)
                            // 		lyric.LangExt = langExt
                            // 		err = embedLyric(audioPath, &lyric, false)
                            // 		err = os.Remove(lyricFileName)
                            // 		lyricWritten++
                            // 	}
                            // }
                            // here we add all downloaded lrc file
                            if let Ok(files) = std::fs::read_dir(&path) {
                                for _f in files.flatten() {
                                    // println!("Name: {}", f.unwrap().path().display())
                                    // println!("Type: {:?}", f.file_type().unwrap());
                                }
                            }

                            let _ = id3_tag.write_to_path(&file_fullname, id3::Version::Id3v24);

                            let _ = tx.send(super::TransferState::Success);
                            sleep(Duration::from_secs(5));
                            let _ = tx.send(super::TransferState::Completed(Some(file_fullname)));
                        }
                        Err(_) => {
                            // This shoudn't happen unless the output format of youtubedl changed
                            let _ = tx.send(super::TransferState::Success);
                            sleep(Duration::from_secs(5));
                            let _ = tx.send(super::TransferState::Completed(None));
                        }
                    }
                    //     let name = p.file_name().and_then(OsStr::to_str).map(|x| x.to_string());
                    //     let duration: Option<Duration> = match mp3_duration::from_path(s) {
                    //         Ok(d) => Some(d),
                    //         Err(_) => Some(Duration::from_secs(0)),
                    //     };

                    //     let id3_tag = match id3::Tag::read_from_path(s) {
                    //     Ok(tag) => tag,
                    //     Err(_) => {
                    // //         let mut t = id3::Tag::new();
                    //         let p: &Path = Path::new(s);
                    //         if let Some(p_base) = p.file_stem() {
                    //             t.set_title(p_base.to_string_lossy());
                    //         }
                    //         let _ = t.write_to_path(p, id3::Version::Id3v24);
                    //         t
                    //     }
                    // };

                    //         let mut tag_song = Tag::new();
                    //         tag_song.set_album(album);
                    //         tag_song.set_title(title);
                    //         tag_song.set_artist(artist);
                    //         if let Ok(l) = lyric {
                    //             tag_song.add_lyrics(Lyrics {
                    //                 lang: String::from("chi"),
                    //                 description: String::from("saved by termusic."),
                    //                 text: l,
                    //             });
                    //         }
                }
                ResultType::IOERROR | ResultType::FAILURE => {
                    let _ = tx.send(super::TransferState::ErrDownload);
                    sleep(Duration::from_secs(5));
                    let _ = tx.send(super::TransferState::Completed(None));
                }
            }
        });
        Ok(())
    }
}
// This just parsing the output from youtubedl to get the audio path
// This is used because we need to get the song name
// example ~/path/to/song/song.mp3
pub fn extract_filepath(output: &str, dir: &str) -> Result<String> {
    let mut filename = String::new();
    filename.push_str(dir);
    filename.push('/');
    // let filename = RE_FILENAME.captures(output).and_then(|cap| {
    //     cap.name("filename").map(|filename | filename + ".mp3")
    // });

    if let Some(cap) = RE_FILENAME.captures(output) {
        filename.push_str(cap.name("name").unwrap().as_str())
    }

    filename.push_str(".mp3");

    Ok(filename)
}

#[cfg(test)]
mod tests {

    use crate::ui::activity::main::youtube_options::extract_filepath;
    use pretty_assertions::assert_eq;
    // use

    #[test]
    fn test_youtube_output_parsing() {
        assert_eq!(extract_filepath(r"sdflsdf [ffmpeg] Destination: 观众说“小哥哥，到饭点了”《干饭人之歌》走，端起饭盆干饭去.mp3 sldflsdfj","/tmp").unwrap(),"/tmp/观众说“小哥哥，到饭点了”《干饭人之歌》走，端起饭盆干饭去.mp3".to_string());
    }
}
