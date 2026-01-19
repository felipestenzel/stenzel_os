//! Japanese Input Method Engine
//!
//! Provides Romaji to Hiragana/Katakana conversion with Kanji candidates.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::ibus::{
    InputMethodEngine, InputMethodType, InputMethodState,
    Candidate, InputEvent, InputResult,
};

/// Japanese input mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JapaneseMode {
    /// Direct input (romaji)
    Direct,
    /// Hiragana
    Hiragana,
    /// Katakana
    Katakana,
    /// Half-width Katakana
    HalfKatakana,
}

impl JapaneseMode {
    pub fn name(&self) -> &'static str {
        match self {
            JapaneseMode::Direct => "Direct",
            JapaneseMode::Hiragana => "Hiragana",
            JapaneseMode::Katakana => "Katakana",
            JapaneseMode::HalfKatakana => "Half Katakana",
        }
    }
}

/// Japanese engine configuration
#[derive(Debug, Clone)]
pub struct JapaneseConfig {
    /// Current input mode
    pub mode: JapaneseMode,
    /// Max candidates
    pub max_candidates: usize,
    /// Auto convert to Kanji
    pub auto_kanji: bool,
}

impl Default for JapaneseConfig {
    fn default() -> Self {
        Self {
            mode: JapaneseMode::Hiragana,
            max_candidates: 9,
            auto_kanji: true,
        }
    }
}

/// Japanese input engine
pub struct JapaneseEngine {
    /// Configuration
    config: JapaneseConfig,
    /// Current state
    state: InputMethodState,
    /// Romaji buffer
    romaji_buffer: String,
    /// Kana buffer (converted)
    kana_buffer: String,
    /// Current candidates
    candidates: Vec<Candidate>,
    /// Selected index
    selected: usize,
    /// Romaji to Hiragana mapping
    romaji_map: BTreeMap<String, String>,
    /// Hiragana to Kanji dictionary
    kanji_dict: BTreeMap<String, Vec<String>>,
}

impl JapaneseEngine {
    /// Create a new Japanese engine
    pub fn new() -> Self {
        let mut engine = Self {
            config: JapaneseConfig::default(),
            state: InputMethodState::Idle,
            romaji_buffer: String::new(),
            kana_buffer: String::new(),
            candidates: Vec::new(),
            selected: 0,
            romaji_map: BTreeMap::new(),
            kanji_dict: BTreeMap::new(),
        };
        engine.load_romaji_table();
        engine.load_kanji_dictionary();
        engine
    }

    /// Load romaji to hiragana conversion table
    fn load_romaji_table(&mut self) {
        // Vowels
        self.romaji_map.insert("a".to_string(), "あ".to_string());
        self.romaji_map.insert("i".to_string(), "い".to_string());
        self.romaji_map.insert("u".to_string(), "う".to_string());
        self.romaji_map.insert("e".to_string(), "え".to_string());
        self.romaji_map.insert("o".to_string(), "お".to_string());

        // K-row
        self.romaji_map.insert("ka".to_string(), "か".to_string());
        self.romaji_map.insert("ki".to_string(), "き".to_string());
        self.romaji_map.insert("ku".to_string(), "く".to_string());
        self.romaji_map.insert("ke".to_string(), "け".to_string());
        self.romaji_map.insert("ko".to_string(), "こ".to_string());

        // S-row
        self.romaji_map.insert("sa".to_string(), "さ".to_string());
        self.romaji_map.insert("si".to_string(), "し".to_string());
        self.romaji_map.insert("shi".to_string(), "し".to_string());
        self.romaji_map.insert("su".to_string(), "す".to_string());
        self.romaji_map.insert("se".to_string(), "せ".to_string());
        self.romaji_map.insert("so".to_string(), "そ".to_string());

        // T-row
        self.romaji_map.insert("ta".to_string(), "た".to_string());
        self.romaji_map.insert("ti".to_string(), "ち".to_string());
        self.romaji_map.insert("chi".to_string(), "ち".to_string());
        self.romaji_map.insert("tu".to_string(), "つ".to_string());
        self.romaji_map.insert("tsu".to_string(), "つ".to_string());
        self.romaji_map.insert("te".to_string(), "て".to_string());
        self.romaji_map.insert("to".to_string(), "と".to_string());

        // N-row
        self.romaji_map.insert("na".to_string(), "な".to_string());
        self.romaji_map.insert("ni".to_string(), "に".to_string());
        self.romaji_map.insert("nu".to_string(), "ぬ".to_string());
        self.romaji_map.insert("ne".to_string(), "ね".to_string());
        self.romaji_map.insert("no".to_string(), "の".to_string());

        // H-row
        self.romaji_map.insert("ha".to_string(), "は".to_string());
        self.romaji_map.insert("hi".to_string(), "ひ".to_string());
        self.romaji_map.insert("hu".to_string(), "ふ".to_string());
        self.romaji_map.insert("fu".to_string(), "ふ".to_string());
        self.romaji_map.insert("he".to_string(), "へ".to_string());
        self.romaji_map.insert("ho".to_string(), "ほ".to_string());

        // M-row
        self.romaji_map.insert("ma".to_string(), "ま".to_string());
        self.romaji_map.insert("mi".to_string(), "み".to_string());
        self.romaji_map.insert("mu".to_string(), "む".to_string());
        self.romaji_map.insert("me".to_string(), "め".to_string());
        self.romaji_map.insert("mo".to_string(), "も".to_string());

        // Y-row
        self.romaji_map.insert("ya".to_string(), "や".to_string());
        self.romaji_map.insert("yu".to_string(), "ゆ".to_string());
        self.romaji_map.insert("yo".to_string(), "よ".to_string());

        // R-row
        self.romaji_map.insert("ra".to_string(), "ら".to_string());
        self.romaji_map.insert("ri".to_string(), "り".to_string());
        self.romaji_map.insert("ru".to_string(), "る".to_string());
        self.romaji_map.insert("re".to_string(), "れ".to_string());
        self.romaji_map.insert("ro".to_string(), "ろ".to_string());

        // W-row
        self.romaji_map.insert("wa".to_string(), "わ".to_string());
        self.romaji_map.insert("wo".to_string(), "を".to_string());

        // N
        self.romaji_map.insert("n".to_string(), "ん".to_string());
        self.romaji_map.insert("nn".to_string(), "ん".to_string());

        // G-row (voiced)
        self.romaji_map.insert("ga".to_string(), "が".to_string());
        self.romaji_map.insert("gi".to_string(), "ぎ".to_string());
        self.romaji_map.insert("gu".to_string(), "ぐ".to_string());
        self.romaji_map.insert("ge".to_string(), "げ".to_string());
        self.romaji_map.insert("go".to_string(), "ご".to_string());

        // Z-row (voiced)
        self.romaji_map.insert("za".to_string(), "ざ".to_string());
        self.romaji_map.insert("zi".to_string(), "じ".to_string());
        self.romaji_map.insert("ji".to_string(), "じ".to_string());
        self.romaji_map.insert("zu".to_string(), "ず".to_string());
        self.romaji_map.insert("ze".to_string(), "ぜ".to_string());
        self.romaji_map.insert("zo".to_string(), "ぞ".to_string());

        // D-row (voiced)
        self.romaji_map.insert("da".to_string(), "だ".to_string());
        self.romaji_map.insert("di".to_string(), "ぢ".to_string());
        self.romaji_map.insert("du".to_string(), "づ".to_string());
        self.romaji_map.insert("de".to_string(), "で".to_string());
        self.romaji_map.insert("do".to_string(), "ど".to_string());

        // B-row (voiced)
        self.romaji_map.insert("ba".to_string(), "ば".to_string());
        self.romaji_map.insert("bi".to_string(), "び".to_string());
        self.romaji_map.insert("bu".to_string(), "ぶ".to_string());
        self.romaji_map.insert("be".to_string(), "べ".to_string());
        self.romaji_map.insert("bo".to_string(), "ぼ".to_string());

        // P-row (half-voiced)
        self.romaji_map.insert("pa".to_string(), "ぱ".to_string());
        self.romaji_map.insert("pi".to_string(), "ぴ".to_string());
        self.romaji_map.insert("pu".to_string(), "ぷ".to_string());
        self.romaji_map.insert("pe".to_string(), "ぺ".to_string());
        self.romaji_map.insert("po".to_string(), "ぽ".to_string());

        // Combined kana (ky, sh, ch, ny, hy, my, ry, gy, j, by, py)
        self.romaji_map.insert("kya".to_string(), "きゃ".to_string());
        self.romaji_map.insert("kyu".to_string(), "きゅ".to_string());
        self.romaji_map.insert("kyo".to_string(), "きょ".to_string());

        self.romaji_map.insert("sha".to_string(), "しゃ".to_string());
        self.romaji_map.insert("shu".to_string(), "しゅ".to_string());
        self.romaji_map.insert("sho".to_string(), "しょ".to_string());
        self.romaji_map.insert("sya".to_string(), "しゃ".to_string());
        self.romaji_map.insert("syu".to_string(), "しゅ".to_string());
        self.romaji_map.insert("syo".to_string(), "しょ".to_string());

        self.romaji_map.insert("cha".to_string(), "ちゃ".to_string());
        self.romaji_map.insert("chu".to_string(), "ちゅ".to_string());
        self.romaji_map.insert("cho".to_string(), "ちょ".to_string());
        self.romaji_map.insert("tya".to_string(), "ちゃ".to_string());
        self.romaji_map.insert("tyu".to_string(), "ちゅ".to_string());
        self.romaji_map.insert("tyo".to_string(), "ちょ".to_string());

        self.romaji_map.insert("nya".to_string(), "にゃ".to_string());
        self.romaji_map.insert("nyu".to_string(), "にゅ".to_string());
        self.romaji_map.insert("nyo".to_string(), "にょ".to_string());

        self.romaji_map.insert("hya".to_string(), "ひゃ".to_string());
        self.romaji_map.insert("hyu".to_string(), "ひゅ".to_string());
        self.romaji_map.insert("hyo".to_string(), "ひょ".to_string());

        self.romaji_map.insert("mya".to_string(), "みゃ".to_string());
        self.romaji_map.insert("myu".to_string(), "みゅ".to_string());
        self.romaji_map.insert("myo".to_string(), "みょ".to_string());

        self.romaji_map.insert("rya".to_string(), "りゃ".to_string());
        self.romaji_map.insert("ryu".to_string(), "りゅ".to_string());
        self.romaji_map.insert("ryo".to_string(), "りょ".to_string());

        self.romaji_map.insert("gya".to_string(), "ぎゃ".to_string());
        self.romaji_map.insert("gyu".to_string(), "ぎゅ".to_string());
        self.romaji_map.insert("gyo".to_string(), "ぎょ".to_string());

        self.romaji_map.insert("ja".to_string(), "じゃ".to_string());
        self.romaji_map.insert("ju".to_string(), "じゅ".to_string());
        self.romaji_map.insert("jo".to_string(), "じょ".to_string());
        self.romaji_map.insert("jya".to_string(), "じゃ".to_string());
        self.romaji_map.insert("jyu".to_string(), "じゅ".to_string());
        self.romaji_map.insert("jyo".to_string(), "じょ".to_string());

        self.romaji_map.insert("bya".to_string(), "びゃ".to_string());
        self.romaji_map.insert("byu".to_string(), "びゅ".to_string());
        self.romaji_map.insert("byo".to_string(), "びょ".to_string());

        self.romaji_map.insert("pya".to_string(), "ぴゃ".to_string());
        self.romaji_map.insert("pyu".to_string(), "ぴゅ".to_string());
        self.romaji_map.insert("pyo".to_string(), "ぴょ".to_string());

        // Small kana
        self.romaji_map.insert("xa".to_string(), "ぁ".to_string());
        self.romaji_map.insert("xi".to_string(), "ぃ".to_string());
        self.romaji_map.insert("xu".to_string(), "ぅ".to_string());
        self.romaji_map.insert("xe".to_string(), "ぇ".to_string());
        self.romaji_map.insert("xo".to_string(), "ぉ".to_string());
        self.romaji_map.insert("xtu".to_string(), "っ".to_string());
        self.romaji_map.insert("xtsu".to_string(), "っ".to_string());
        self.romaji_map.insert("xya".to_string(), "ゃ".to_string());
        self.romaji_map.insert("xyu".to_string(), "ゅ".to_string());
        self.romaji_map.insert("xyo".to_string(), "ょ".to_string());

        // Double consonant (っ)
        for c in ['k', 's', 't', 'h', 'f', 'n', 'm', 'y', 'r', 'w', 'g', 'z', 'd', 'b', 'p', 'c', 'j'].iter() {
            let double = alloc::format!("{}{}", c, c);
            self.romaji_map.insert(double.clone(), "っ".to_string());
        }
    }

    /// Load basic kanji dictionary
    fn load_kanji_dictionary(&mut self) {
        // Common words - hiragana to kanji
        self.add_kanji("あい", &["愛", "合い", "会い"]);
        self.add_kanji("あう", &["会う", "合う", "遭う"]);
        self.add_kanji("あか", &["赤", "明か"]);
        self.add_kanji("あき", &["秋", "空き"]);
        self.add_kanji("あさ", &["朝", "麻"]);
        self.add_kanji("あし", &["足", "脚"]);
        self.add_kanji("あたま", &["頭"]);
        self.add_kanji("あたらしい", &["新しい"]);
        self.add_kanji("あつい", &["暑い", "熱い", "厚い"]);
        self.add_kanji("あと", &["後", "跡"]);
        self.add_kanji("あに", &["兄"]);
        self.add_kanji("あね", &["姉"]);
        self.add_kanji("あめ", &["雨", "飴"]);
        self.add_kanji("あるく", &["歩く"]);

        self.add_kanji("いえ", &["家"]);
        self.add_kanji("いく", &["行く", "逝く"]);
        self.add_kanji("いし", &["石", "医師", "意思"]);
        self.add_kanji("いそがしい", &["忙しい"]);
        self.add_kanji("いち", &["一", "位置"]);
        self.add_kanji("いぬ", &["犬"]);
        self.add_kanji("いま", &["今", "居間"]);
        self.add_kanji("いみ", &["意味"]);
        self.add_kanji("いる", &["居る", "要る"]);

        self.add_kanji("うえ", &["上"]);
        self.add_kanji("うごく", &["動く"]);
        self.add_kanji("うた", &["歌"]);
        self.add_kanji("うみ", &["海"]);
        self.add_kanji("うる", &["売る"]);

        self.add_kanji("えき", &["駅", "液"]);
        self.add_kanji("えらぶ", &["選ぶ"]);

        self.add_kanji("おおきい", &["大きい"]);
        self.add_kanji("おかね", &["お金"]);
        self.add_kanji("おく", &["置く", "奥"]);
        self.add_kanji("おくる", &["送る", "贈る"]);
        self.add_kanji("おこなう", &["行う"]);
        self.add_kanji("おしえる", &["教える"]);
        self.add_kanji("おと", &["音"]);
        self.add_kanji("おとこ", &["男"]);
        self.add_kanji("おなじ", &["同じ"]);
        self.add_kanji("おもう", &["思う"]);
        self.add_kanji("おんな", &["女"]);

        self.add_kanji("かいしゃ", &["会社"]);
        self.add_kanji("かう", &["買う", "飼う"]);
        self.add_kanji("かお", &["顔"]);
        self.add_kanji("かく", &["書く", "描く"]);
        self.add_kanji("かぜ", &["風", "風邪"]);
        self.add_kanji("かた", &["方", "肩"]);
        self.add_kanji("かみ", &["紙", "髪", "神"]);
        self.add_kanji("からだ", &["体"]);
        self.add_kanji("かわ", &["川", "皮"]);
        self.add_kanji("かんがえる", &["考える"]);

        self.add_kanji("き", &["木", "気"]);
        self.add_kanji("きく", &["聞く", "効く"]);
        self.add_kanji("きた", &["北"]);
        self.add_kanji("きょう", &["今日", "教"]);
        self.add_kanji("きる", &["切る", "着る"]);

        self.add_kanji("くに", &["国"]);
        self.add_kanji("くる", &["来る"]);
        self.add_kanji("くるま", &["車"]);
        self.add_kanji("くろい", &["黒い"]);

        self.add_kanji("けいたい", &["携帯"]);
        self.add_kanji("けっこん", &["結婚"]);

        self.add_kanji("こえ", &["声"]);
        self.add_kanji("ここ", &["此処"]);
        self.add_kanji("こころ", &["心"]);
        self.add_kanji("こたえ", &["答え"]);
        self.add_kanji("ことば", &["言葉"]);
        self.add_kanji("この", &["此の"]);
        self.add_kanji("こんにちは", &["今日は"]);

        self.add_kanji("さかな", &["魚"]);
        self.add_kanji("さき", &["先"]);
        self.add_kanji("さく", &["咲く"]);
        self.add_kanji("さむい", &["寒い"]);

        self.add_kanji("しごと", &["仕事"]);
        self.add_kanji("した", &["下"]);
        self.add_kanji("しぬ", &["死ぬ"]);
        self.add_kanji("しま", &["島"]);
        self.add_kanji("しる", &["知る"]);
        self.add_kanji("しろい", &["白い"]);

        self.add_kanji("すき", &["好き"]);
        self.add_kanji("すこし", &["少し"]);
        self.add_kanji("すむ", &["住む"]);
        self.add_kanji("する", &["為る"]);

        self.add_kanji("せかい", &["世界"]);
        self.add_kanji("せんせい", &["先生"]);

        self.add_kanji("そと", &["外"]);
        self.add_kanji("そら", &["空"]);

        self.add_kanji("たかい", &["高い"]);
        self.add_kanji("たつ", &["立つ"]);
        self.add_kanji("たべる", &["食べる"]);

        self.add_kanji("ちいさい", &["小さい"]);
        self.add_kanji("ちから", &["力"]);

        self.add_kanji("つかう", &["使う"]);
        self.add_kanji("つくる", &["作る", "造る"]);

        self.add_kanji("て", &["手"]);
        self.add_kanji("てがみ", &["手紙"]);
        self.add_kanji("でる", &["出る"]);
        self.add_kanji("でんき", &["電気"]);
        self.add_kanji("でんしゃ", &["電車"]);
        self.add_kanji("でんわ", &["電話"]);

        self.add_kanji("とき", &["時"]);
        self.add_kanji("ところ", &["所"]);
        self.add_kanji("とし", &["年", "都市"]);
        self.add_kanji("とまる", &["止まる", "泊まる"]);
        self.add_kanji("ともだち", &["友達"]);
        self.add_kanji("とり", &["鳥"]);
        self.add_kanji("とる", &["取る", "撮る"]);

        self.add_kanji("なか", &["中"]);
        self.add_kanji("ながい", &["長い"]);
        self.add_kanji("なつ", &["夏"]);
        self.add_kanji("なに", &["何"]);
        self.add_kanji("なまえ", &["名前"]);
        self.add_kanji("なる", &["成る", "鳴る"]);

        self.add_kanji("にし", &["西"]);
        self.add_kanji("にほん", &["日本"]);
        self.add_kanji("にんげん", &["人間"]);

        self.add_kanji("ねこ", &["猫"]);
        self.add_kanji("ねる", &["寝る"]);

        self.add_kanji("のむ", &["飲む"]);
        self.add_kanji("のる", &["乗る"]);

        self.add_kanji("はいる", &["入る"]);
        self.add_kanji("はじめる", &["始める"]);
        self.add_kanji("はしる", &["走る"]);
        self.add_kanji("はたらく", &["働く"]);
        self.add_kanji("はな", &["花", "鼻"]);
        self.add_kanji("はなす", &["話す", "離す"]);
        self.add_kanji("はは", &["母"]);
        self.add_kanji("はやい", &["早い", "速い"]);
        self.add_kanji("はる", &["春"]);

        self.add_kanji("ひがし", &["東"]);
        self.add_kanji("ひかり", &["光"]);
        self.add_kanji("ひと", &["人"]);

        self.add_kanji("ふゆ", &["冬"]);
        self.add_kanji("ふるい", &["古い"]);

        self.add_kanji("へや", &["部屋"]);

        self.add_kanji("ほし", &["星"]);
        self.add_kanji("ほん", &["本"]);

        self.add_kanji("まえ", &["前"]);
        self.add_kanji("まち", &["町", "街"]);
        self.add_kanji("まつ", &["待つ"]);
        self.add_kanji("まど", &["窓"]);

        self.add_kanji("みえる", &["見える"]);
        self.add_kanji("みぎ", &["右"]);
        self.add_kanji("みず", &["水"]);
        self.add_kanji("みせ", &["店"]);
        self.add_kanji("みち", &["道"]);
        self.add_kanji("みなみ", &["南"]);
        self.add_kanji("みみ", &["耳"]);
        self.add_kanji("みる", &["見る"]);

        self.add_kanji("むすめ", &["娘"]);

        self.add_kanji("め", &["目"]);

        self.add_kanji("もつ", &["持つ"]);
        self.add_kanji("もの", &["物"]);
        self.add_kanji("もり", &["森"]);

        self.add_kanji("やすい", &["安い", "易い"]);
        self.add_kanji("やすむ", &["休む"]);
        self.add_kanji("やま", &["山"]);

        self.add_kanji("ゆき", &["雪"]);

        self.add_kanji("よい", &["良い"]);
        self.add_kanji("よむ", &["読む"]);
        self.add_kanji("よる", &["夜"]);

        self.add_kanji("わかる", &["分かる"]);
        self.add_kanji("わたし", &["私"]);
    }

    /// Add kanji entry
    fn add_kanji(&mut self, hiragana: &str, kanji: &[&str]) {
        let entries: Vec<String> = kanji.iter().map(|s| s.to_string()).collect();
        self.kanji_dict.insert(hiragana.to_string(), entries);
    }

    /// Convert romaji to kana
    fn romaji_to_kana(&self, romaji: &str) -> (String, String) {
        let mut kana = String::new();
        let mut remaining = String::new();
        let mut i = 0;
        let chars: Vec<char> = romaji.chars().collect();

        while i < chars.len() {
            let mut matched = false;

            // Try longer matches first (up to 4 chars)
            for len in (1..=4.min(chars.len() - i)).rev() {
                let substr: String = chars[i..i + len].iter().collect();

                if let Some(kana_char) = self.romaji_map.get(&substr) {
                    kana.push_str(kana_char);
                    i += len;
                    matched = true;
                    break;
                }
            }

            if !matched {
                remaining.push(chars[i]);
                i += 1;
            }
        }

        // Check if remaining could be start of a valid romaji
        if !remaining.is_empty() {
            let could_continue = self.romaji_map.keys().any(|k| k.starts_with(&remaining));
            if could_continue {
                return (kana, remaining);
            }
        }

        (kana, remaining)
    }

    /// Convert hiragana to katakana
    fn hiragana_to_katakana(hiragana: &str) -> String {
        hiragana.chars().map(|c| {
            let code = c as u32;
            if code >= 0x3041 && code <= 0x3096 {
                char::from_u32(code + 0x60).unwrap_or(c)
            } else {
                c
            }
        }).collect()
    }

    /// Look up kanji candidates
    fn lookup_kanji(&self, hiragana: &str) -> Vec<Candidate> {
        let mut candidates = Vec::new();

        // Add hiragana as first option
        candidates.push(Candidate {
            text: hiragana.to_string(),
            label: None,
            annotation: Some("ひらがな".to_string()),
            score: 100,
        });

        // Add katakana
        let katakana = Self::hiragana_to_katakana(hiragana);
        candidates.push(Candidate {
            text: katakana,
            label: None,
            annotation: Some("カタカナ".to_string()),
            score: 99,
        });

        // Look up kanji
        if let Some(kanji_list) = self.kanji_dict.get(hiragana) {
            for (i, kanji) in kanji_list.iter().enumerate() {
                candidates.push(Candidate {
                    text: kanji.clone(),
                    label: None,
                    annotation: Some(hiragana.to_string()),
                    score: (90 - i) as u32,
                });
            }
        }

        candidates.truncate(self.config.max_candidates);
        candidates
    }

    /// Update state and candidates
    fn update(&mut self) {
        if self.kana_buffer.is_empty() && self.romaji_buffer.is_empty() {
            self.candidates.clear();
            self.state = InputMethodState::Idle;
        } else if !self.kana_buffer.is_empty() {
            self.candidates = self.lookup_kanji(&self.kana_buffer);
            self.selected = 0;
            self.state = InputMethodState::Selecting;
        } else {
            self.state = InputMethodState::Composing;
        }
    }
}

impl Default for JapaneseEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InputMethodEngine for JapaneseEngine {
    fn im_type(&self) -> InputMethodType {
        InputMethodType::Japanese
    }

    fn process_key(&mut self, event: InputEvent) -> InputResult {
        if !event.is_press {
            return InputResult::NotHandled;
        }

        if event.modifiers.ctrl || event.modifiers.alt {
            return InputResult::NotHandled;
        }

        let ch = match event.character {
            Some(c) => c,
            None => return InputResult::NotHandled,
        };

        match ch {
            'a'..='z' | 'A'..='Z' => {
                self.romaji_buffer.push(ch.to_ascii_lowercase());

                // Try to convert romaji to kana
                let (converted, remaining) = self.romaji_to_kana(&self.romaji_buffer);
                if !converted.is_empty() {
                    self.kana_buffer.push_str(&converted);
                }
                self.romaji_buffer = remaining;

                self.update();

                let preedit = alloc::format!("{}{}", self.kana_buffer, self.romaji_buffer);
                if !self.candidates.is_empty() {
                    return InputResult::ShowCandidates(self.candidates.clone());
                }
                let cursor = preedit.len();
                return InputResult::Preedit {
                    text: preedit,
                    cursor,
                };
            }

            '1'..='9' if !self.candidates.is_empty() => {
                let idx = (ch as usize) - ('1' as usize);
                if idx < self.candidates.len() {
                    let text = self.candidates[idx].text.clone();
                    self.reset();
                    return InputResult::Commit(text);
                }
            }

            ' ' => {
                // Space commits first candidate
                if !self.candidates.is_empty() {
                    let text = self.candidates[self.selected].text.clone();
                    self.reset();
                    return InputResult::Commit(text);
                } else if !self.kana_buffer.is_empty() || !self.romaji_buffer.is_empty() {
                    let text = alloc::format!("{}{}", self.kana_buffer, self.romaji_buffer);
                    self.reset();
                    return InputResult::Commit(text);
                }
            }

            '\x08' | '\x7f' => {
                if !self.romaji_buffer.is_empty() {
                    self.romaji_buffer.pop();
                } else if !self.kana_buffer.is_empty() {
                    self.kana_buffer.pop();
                }
                self.update();

                if self.kana_buffer.is_empty() && self.romaji_buffer.is_empty() {
                    return InputResult::HideCandidates;
                }

                let preedit = alloc::format!("{}{}", self.kana_buffer, self.romaji_buffer);
                if !self.candidates.is_empty() {
                    return InputResult::ShowCandidates(self.candidates.clone());
                }
                let cursor = preedit.len();
                return InputResult::Preedit {
                    text: preedit,
                    cursor,
                };
            }

            '\x1b' => {
                if !self.kana_buffer.is_empty() || !self.romaji_buffer.is_empty() {
                    self.reset();
                    return InputResult::HideCandidates;
                }
            }

            '\r' | '\n' => {
                if !self.kana_buffer.is_empty() || !self.romaji_buffer.is_empty() {
                    let text = alloc::format!("{}{}", self.kana_buffer, self.romaji_buffer);
                    self.reset();
                    return InputResult::Commit(text);
                }
            }

            _ => {}
        }

        InputResult::NotHandled
    }

    fn preedit(&self) -> &str {
        &self.kana_buffer
    }

    fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    fn state(&self) -> InputMethodState {
        self.state
    }

    fn reset(&mut self) {
        self.romaji_buffer.clear();
        self.kana_buffer.clear();
        self.candidates.clear();
        self.selected = 0;
        self.state = InputMethodState::Idle;
    }

    fn selected_index(&self) -> usize {
        self.selected
    }

    fn select_candidate(&mut self, index: usize) -> Option<String> {
        if index < self.candidates.len() {
            let text = self.candidates[index].text.clone();
            self.reset();
            Some(text)
        } else {
            None
        }
    }

    fn move_up(&mut self) {
        if !self.candidates.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    fn move_down(&mut self) {
        if !self.candidates.is_empty() && self.selected < self.candidates.len() - 1 {
            self.selected += 1;
        }
    }

    fn page_up(&mut self) {
        if !self.candidates.is_empty() {
            self.selected = self.selected.saturating_sub(self.config.max_candidates);
        }
    }

    fn page_down(&mut self) {
        if !self.candidates.is_empty() {
            let new_sel = self.selected + self.config.max_candidates;
            self.selected = new_sel.min(self.candidates.len() - 1);
        }
    }

    fn commit(&mut self) -> Option<String> {
        if !self.candidates.is_empty() {
            let text = self.candidates[self.selected].text.clone();
            self.reset();
            Some(text)
        } else if !self.kana_buffer.is_empty() || !self.romaji_buffer.is_empty() {
            let text = alloc::format!("{}{}", self.kana_buffer, self.romaji_buffer);
            self.reset();
            Some(text)
        } else {
            None
        }
    }

    fn cancel(&mut self) {
        self.reset();
    }
}
