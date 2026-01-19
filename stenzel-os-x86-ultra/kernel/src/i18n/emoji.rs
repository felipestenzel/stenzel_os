//! Emoji Picker Input Method
//!
//! Provides an emoji picker with search and category browsing.

#![allow(dead_code)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::BTreeMap;

/// Emoji category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmojiCategory {
    Recent,
    Smileys,
    People,
    Animals,
    Food,
    Travel,
    Activities,
    Objects,
    Symbols,
    Flags,
}

impl EmojiCategory {
    pub fn name(&self) -> &'static str {
        match self {
            EmojiCategory::Recent => "Recent",
            EmojiCategory::Smileys => "Smileys & Emotion",
            EmojiCategory::People => "People & Body",
            EmojiCategory::Animals => "Animals & Nature",
            EmojiCategory::Food => "Food & Drink",
            EmojiCategory::Travel => "Travel & Places",
            EmojiCategory::Activities => "Activities",
            EmojiCategory::Objects => "Objects",
            EmojiCategory::Symbols => "Symbols",
            EmojiCategory::Flags => "Flags",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            EmojiCategory::Recent => "🕐",
            EmojiCategory::Smileys => "😀",
            EmojiCategory::People => "👋",
            EmojiCategory::Animals => "🐶",
            EmojiCategory::Food => "🍔",
            EmojiCategory::Travel => "✈️",
            EmojiCategory::Activities => "⚽",
            EmojiCategory::Objects => "💡",
            EmojiCategory::Symbols => "❤️",
            EmojiCategory::Flags => "🏳️",
        }
    }

    pub fn all() -> &'static [EmojiCategory] {
        &[
            EmojiCategory::Recent,
            EmojiCategory::Smileys,
            EmojiCategory::People,
            EmojiCategory::Animals,
            EmojiCategory::Food,
            EmojiCategory::Travel,
            EmojiCategory::Activities,
            EmojiCategory::Objects,
            EmojiCategory::Symbols,
            EmojiCategory::Flags,
        ]
    }
}

/// Single emoji entry
#[derive(Debug, Clone)]
pub struct Emoji {
    pub emoji: &'static str,
    pub name: &'static str,
    pub keywords: &'static [&'static str],
    pub category: EmojiCategory,
}

impl Emoji {
    pub const fn new(
        emoji: &'static str,
        name: &'static str,
        keywords: &'static [&'static str],
        category: EmojiCategory,
    ) -> Self {
        Self { emoji, name, keywords, category }
    }

    pub fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        if self.name.to_lowercase().contains(&query_lower) {
            return true;
        }
        for kw in self.keywords {
            if kw.to_lowercase().contains(&query_lower) {
                return true;
            }
        }
        false
    }
}

/// Emoji picker configuration
#[derive(Debug, Clone)]
pub struct EmojiPickerConfig {
    pub max_recent: usize,
    pub columns: usize,
    pub show_names: bool,
    pub skin_tone: SkinTone,
}

impl Default for EmojiPickerConfig {
    fn default() -> Self {
        Self {
            max_recent: 30,
            columns: 8,
            show_names: true,
            skin_tone: SkinTone::Default,
        }
    }
}

/// Skin tone modifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkinTone {
    Default,
    Light,
    MediumLight,
    Medium,
    MediumDark,
    Dark,
}

impl SkinTone {
    pub fn modifier(&self) -> &'static str {
        match self {
            SkinTone::Default => "",
            SkinTone::Light => "\u{1F3FB}",
            SkinTone::MediumLight => "\u{1F3FC}",
            SkinTone::Medium => "\u{1F3FD}",
            SkinTone::MediumDark => "\u{1F3FE}",
            SkinTone::Dark => "\u{1F3FF}",
        }
    }
}

/// Emoji picker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerState {
    Closed,
    Browse,
    Search,
}

/// Emoji picker
pub struct EmojiPicker {
    config: EmojiPickerConfig,
    state: PickerState,
    current_category: EmojiCategory,
    search_query: String,
    search_results: Vec<usize>,
    selected_index: usize,
    recent: Vec<usize>,
    emojis: Vec<Emoji>,
    category_map: BTreeMap<u8, Vec<usize>>,
}

impl EmojiPicker {
    pub fn new() -> Self {
        let mut picker = Self {
            config: EmojiPickerConfig::default(),
            state: PickerState::Closed,
            current_category: EmojiCategory::Smileys,
            search_query: String::new(),
            search_results: Vec::new(),
            selected_index: 0,
            recent: Vec::new(),
            emojis: Vec::new(),
            category_map: BTreeMap::new(),
        };
        picker.load_emojis();
        picker
    }

    fn load_emojis(&mut self) {
        // Smileys & Emotion
        self.add_emoji("😀", "grinning face", &["smile", "happy"], EmojiCategory::Smileys);
        self.add_emoji("😃", "grinning face with big eyes", &["happy", "joy"], EmojiCategory::Smileys);
        self.add_emoji("😄", "grinning face with smiling eyes", &["happy", "laugh"], EmojiCategory::Smileys);
        self.add_emoji("😁", "beaming face with smiling eyes", &["grin"], EmojiCategory::Smileys);
        self.add_emoji("😅", "grinning face with sweat", &["hot", "nervous"], EmojiCategory::Smileys);
        self.add_emoji("🤣", "rolling on the floor laughing", &["lol", "rofl"], EmojiCategory::Smileys);
        self.add_emoji("😂", "face with tears of joy", &["lol", "laugh", "cry"], EmojiCategory::Smileys);
        self.add_emoji("🙂", "slightly smiling face", &["smile"], EmojiCategory::Smileys);
        self.add_emoji("😉", "winking face", &["wink"], EmojiCategory::Smileys);
        self.add_emoji("😊", "smiling face with smiling eyes", &["blush", "shy"], EmojiCategory::Smileys);
        self.add_emoji("😇", "smiling face with halo", &["angel", "innocent"], EmojiCategory::Smileys);
        self.add_emoji("🥰", "smiling face with hearts", &["love", "adore"], EmojiCategory::Smileys);
        self.add_emoji("😍", "smiling face with heart-eyes", &["love", "crush"], EmojiCategory::Smileys);
        self.add_emoji("🤩", "star-struck", &["wow", "amazing"], EmojiCategory::Smileys);
        self.add_emoji("😘", "face blowing a kiss", &["kiss", "love"], EmojiCategory::Smileys);
        self.add_emoji("😗", "kissing face", &["kiss"], EmojiCategory::Smileys);
        self.add_emoji("😚", "kissing face with closed eyes", &["kiss"], EmojiCategory::Smileys);
        self.add_emoji("😋", "face savoring food", &["yum", "delicious"], EmojiCategory::Smileys);
        self.add_emoji("😛", "face with tongue", &["tongue", "playful"], EmojiCategory::Smileys);
        self.add_emoji("😜", "winking face with tongue", &["crazy", "playful"], EmojiCategory::Smileys);
        self.add_emoji("🤪", "zany face", &["crazy", "wild"], EmojiCategory::Smileys);
        self.add_emoji("😎", "smiling face with sunglasses", &["cool", "sunglasses"], EmojiCategory::Smileys);
        self.add_emoji("🤓", "nerd face", &["nerd", "geek", "glasses"], EmojiCategory::Smileys);
        self.add_emoji("🧐", "face with monocle", &["thinking", "smart"], EmojiCategory::Smileys);
        self.add_emoji("🤔", "thinking face", &["think", "hmm"], EmojiCategory::Smileys);
        self.add_emoji("🤨", "face with raised eyebrow", &["suspicious", "skeptical"], EmojiCategory::Smileys);
        self.add_emoji("😐", "neutral face", &["meh", "blank"], EmojiCategory::Smileys);
        self.add_emoji("😑", "expressionless face", &["blank", "meh"], EmojiCategory::Smileys);
        self.add_emoji("😶", "face without mouth", &["silent", "speechless"], EmojiCategory::Smileys);
        self.add_emoji("😏", "smirking face", &["smirk", "smug"], EmojiCategory::Smileys);
        self.add_emoji("😒", "unamused face", &["meh", "annoyed"], EmojiCategory::Smileys);
        self.add_emoji("🙄", "face with rolling eyes", &["eyeroll", "whatever"], EmojiCategory::Smileys);
        self.add_emoji("😬", "grimacing face", &["awkward", "nervous"], EmojiCategory::Smileys);
        self.add_emoji("😮‍💨", "face exhaling", &["sigh", "relief"], EmojiCategory::Smileys);
        self.add_emoji("🤥", "lying face", &["lie", "pinocchio"], EmojiCategory::Smileys);
        self.add_emoji("😌", "relieved face", &["relieved", "content"], EmojiCategory::Smileys);
        self.add_emoji("😔", "pensive face", &["sad", "thoughtful"], EmojiCategory::Smileys);
        self.add_emoji("😪", "sleepy face", &["tired", "sleep"], EmojiCategory::Smileys);
        self.add_emoji("🤤", "drooling face", &["drool", "yum"], EmojiCategory::Smileys);
        self.add_emoji("😴", "sleeping face", &["sleep", "zzz"], EmojiCategory::Smileys);
        self.add_emoji("😷", "face with medical mask", &["sick", "mask"], EmojiCategory::Smileys);
        self.add_emoji("🤒", "face with thermometer", &["sick", "fever"], EmojiCategory::Smileys);
        self.add_emoji("🤕", "face with head-bandage", &["hurt", "injured"], EmojiCategory::Smileys);
        self.add_emoji("🤢", "nauseated face", &["sick", "vomit"], EmojiCategory::Smileys);
        self.add_emoji("🤮", "face vomiting", &["sick", "vomit"], EmojiCategory::Smileys);
        self.add_emoji("😵", "face with crossed-out eyes", &["dizzy", "dead"], EmojiCategory::Smileys);
        self.add_emoji("🥴", "woozy face", &["drunk", "tipsy"], EmojiCategory::Smileys);
        self.add_emoji("😱", "face screaming in fear", &["scared", "horror"], EmojiCategory::Smileys);
        self.add_emoji("😨", "fearful face", &["scared", "fear"], EmojiCategory::Smileys);
        self.add_emoji("😰", "anxious face with sweat", &["nervous", "anxious"], EmojiCategory::Smileys);
        self.add_emoji("😥", "sad but relieved face", &["disappointed", "relieved"], EmojiCategory::Smileys);
        self.add_emoji("😢", "crying face", &["sad", "tear"], EmojiCategory::Smileys);
        self.add_emoji("😭", "loudly crying face", &["sob", "cry"], EmojiCategory::Smileys);
        self.add_emoji("😤", "face with steam from nose", &["angry", "frustrated"], EmojiCategory::Smileys);
        self.add_emoji("😠", "angry face", &["angry", "mad"], EmojiCategory::Smileys);
        self.add_emoji("😡", "pouting face", &["angry", "rage"], EmojiCategory::Smileys);
        self.add_emoji("🤬", "face with symbols on mouth", &["swear", "curse"], EmojiCategory::Smileys);
        self.add_emoji("💀", "skull", &["dead", "death"], EmojiCategory::Smileys);
        self.add_emoji("👻", "ghost", &["halloween", "spooky"], EmojiCategory::Smileys);
        self.add_emoji("💩", "pile of poo", &["poop", "shit"], EmojiCategory::Smileys);

        // People & Body
        self.add_emoji("👋", "waving hand", &["wave", "hello", "bye"], EmojiCategory::People);
        self.add_emoji("🤚", "raised back of hand", &["stop"], EmojiCategory::People);
        self.add_emoji("✋", "raised hand", &["stop", "high five"], EmojiCategory::People);
        self.add_emoji("🖖", "vulcan salute", &["spock", "star trek"], EmojiCategory::People);
        self.add_emoji("👌", "OK hand", &["ok", "perfect"], EmojiCategory::People);
        self.add_emoji("🤌", "pinched fingers", &["italian", "chef"], EmojiCategory::People);
        self.add_emoji("✌️", "victory hand", &["peace", "v"], EmojiCategory::People);
        self.add_emoji("🤞", "crossed fingers", &["luck", "hope"], EmojiCategory::People);
        self.add_emoji("🤟", "love-you gesture", &["ily", "love"], EmojiCategory::People);
        self.add_emoji("🤘", "sign of the horns", &["rock", "metal"], EmojiCategory::People);
        self.add_emoji("🤙", "call me hand", &["call", "shaka"], EmojiCategory::People);
        self.add_emoji("👈", "backhand index pointing left", &["left"], EmojiCategory::People);
        self.add_emoji("👉", "backhand index pointing right", &["right"], EmojiCategory::People);
        self.add_emoji("👆", "backhand index pointing up", &["up"], EmojiCategory::People);
        self.add_emoji("👇", "backhand index pointing down", &["down"], EmojiCategory::People);
        self.add_emoji("☝️", "index pointing up", &["one", "up"], EmojiCategory::People);
        self.add_emoji("👍", "thumbs up", &["like", "yes", "good"], EmojiCategory::People);
        self.add_emoji("👎", "thumbs down", &["dislike", "no", "bad"], EmojiCategory::People);
        self.add_emoji("✊", "raised fist", &["power", "punch"], EmojiCategory::People);
        self.add_emoji("👊", "oncoming fist", &["punch", "fist bump"], EmojiCategory::People);
        self.add_emoji("🤛", "left-facing fist", &["fist bump"], EmojiCategory::People);
        self.add_emoji("🤜", "right-facing fist", &["fist bump"], EmojiCategory::People);
        self.add_emoji("👏", "clapping hands", &["applause", "clap"], EmojiCategory::People);
        self.add_emoji("🙌", "raising hands", &["hooray", "celebrate"], EmojiCategory::People);
        self.add_emoji("🤝", "handshake", &["deal", "agreement"], EmojiCategory::People);
        self.add_emoji("🙏", "folded hands", &["pray", "please", "thanks"], EmojiCategory::People);
        self.add_emoji("💪", "flexed biceps", &["strong", "muscle"], EmojiCategory::People);

        // Animals & Nature
        self.add_emoji("🐶", "dog face", &["dog", "puppy", "pet"], EmojiCategory::Animals);
        self.add_emoji("🐱", "cat face", &["cat", "kitten", "pet"], EmojiCategory::Animals);
        self.add_emoji("🐭", "mouse face", &["mouse", "rat"], EmojiCategory::Animals);
        self.add_emoji("🐹", "hamster", &["hamster", "pet"], EmojiCategory::Animals);
        self.add_emoji("🐰", "rabbit face", &["bunny", "rabbit"], EmojiCategory::Animals);
        self.add_emoji("🦊", "fox", &["fox"], EmojiCategory::Animals);
        self.add_emoji("🐻", "bear", &["bear"], EmojiCategory::Animals);
        self.add_emoji("🐼", "panda", &["panda", "bear"], EmojiCategory::Animals);
        self.add_emoji("🐨", "koala", &["koala"], EmojiCategory::Animals);
        self.add_emoji("🐯", "tiger face", &["tiger"], EmojiCategory::Animals);
        self.add_emoji("🦁", "lion", &["lion", "king"], EmojiCategory::Animals);
        self.add_emoji("🐮", "cow face", &["cow", "moo"], EmojiCategory::Animals);
        self.add_emoji("🐷", "pig face", &["pig", "oink"], EmojiCategory::Animals);
        self.add_emoji("🐸", "frog", &["frog", "toad"], EmojiCategory::Animals);
        self.add_emoji("🐵", "monkey face", &["monkey"], EmojiCategory::Animals);
        self.add_emoji("🐔", "chicken", &["chicken", "hen"], EmojiCategory::Animals);
        self.add_emoji("🐧", "penguin", &["penguin"], EmojiCategory::Animals);
        self.add_emoji("🐦", "bird", &["bird"], EmojiCategory::Animals);
        self.add_emoji("🦆", "duck", &["duck", "quack"], EmojiCategory::Animals);
        self.add_emoji("🦅", "eagle", &["eagle", "bird"], EmojiCategory::Animals);
        self.add_emoji("🦉", "owl", &["owl", "bird"], EmojiCategory::Animals);
        self.add_emoji("🦇", "bat", &["bat"], EmojiCategory::Animals);
        self.add_emoji("🐺", "wolf", &["wolf"], EmojiCategory::Animals);
        self.add_emoji("🐗", "boar", &["boar", "pig"], EmojiCategory::Animals);
        self.add_emoji("🐴", "horse face", &["horse"], EmojiCategory::Animals);
        self.add_emoji("🦄", "unicorn", &["unicorn", "magic"], EmojiCategory::Animals);
        self.add_emoji("🐝", "honeybee", &["bee", "honey"], EmojiCategory::Animals);
        self.add_emoji("🐛", "bug", &["bug", "insect"], EmojiCategory::Animals);
        self.add_emoji("🦋", "butterfly", &["butterfly"], EmojiCategory::Animals);
        self.add_emoji("🐌", "snail", &["snail", "slow"], EmojiCategory::Animals);
        self.add_emoji("🐙", "octopus", &["octopus"], EmojiCategory::Animals);
        self.add_emoji("🦑", "squid", &["squid"], EmojiCategory::Animals);
        self.add_emoji("🦐", "shrimp", &["shrimp", "prawn"], EmojiCategory::Animals);
        self.add_emoji("🦀", "crab", &["crab"], EmojiCategory::Animals);
        self.add_emoji("🐠", "tropical fish", &["fish"], EmojiCategory::Animals);
        self.add_emoji("🐟", "fish", &["fish"], EmojiCategory::Animals);
        self.add_emoji("🐬", "dolphin", &["dolphin"], EmojiCategory::Animals);
        self.add_emoji("🐳", "spouting whale", &["whale"], EmojiCategory::Animals);
        self.add_emoji("🦈", "shark", &["shark"], EmojiCategory::Animals);
        self.add_emoji("🐊", "crocodile", &["crocodile", "alligator"], EmojiCategory::Animals);
        self.add_emoji("🐢", "turtle", &["turtle"], EmojiCategory::Animals);
        self.add_emoji("🦎", "lizard", &["lizard"], EmojiCategory::Animals);
        self.add_emoji("🐍", "snake", &["snake"], EmojiCategory::Animals);
        self.add_emoji("🦖", "T-Rex", &["dinosaur", "trex"], EmojiCategory::Animals);
        self.add_emoji("🦕", "sauropod", &["dinosaur"], EmojiCategory::Animals);
        self.add_emoji("🌸", "cherry blossom", &["flower", "spring"], EmojiCategory::Animals);
        self.add_emoji("🌹", "rose", &["flower", "love"], EmojiCategory::Animals);
        self.add_emoji("🌺", "hibiscus", &["flower"], EmojiCategory::Animals);
        self.add_emoji("🌻", "sunflower", &["flower", "sun"], EmojiCategory::Animals);
        self.add_emoji("🌲", "evergreen tree", &["tree", "christmas"], EmojiCategory::Animals);
        self.add_emoji("🌳", "deciduous tree", &["tree"], EmojiCategory::Animals);
        self.add_emoji("🌴", "palm tree", &["tree", "beach", "tropical"], EmojiCategory::Animals);
        self.add_emoji("🌵", "cactus", &["desert"], EmojiCategory::Animals);
        self.add_emoji("🍀", "four leaf clover", &["luck", "irish"], EmojiCategory::Animals);

        // Food & Drink
        self.add_emoji("🍎", "red apple", &["apple", "fruit"], EmojiCategory::Food);
        self.add_emoji("🍊", "tangerine", &["orange", "fruit"], EmojiCategory::Food);
        self.add_emoji("🍋", "lemon", &["lemon", "fruit"], EmojiCategory::Food);
        self.add_emoji("🍌", "banana", &["banana", "fruit"], EmojiCategory::Food);
        self.add_emoji("🍉", "watermelon", &["fruit", "summer"], EmojiCategory::Food);
        self.add_emoji("🍇", "grapes", &["fruit", "wine"], EmojiCategory::Food);
        self.add_emoji("🍓", "strawberry", &["fruit", "berry"], EmojiCategory::Food);
        self.add_emoji("🍑", "peach", &["fruit"], EmojiCategory::Food);
        self.add_emoji("🍒", "cherries", &["fruit", "cherry"], EmojiCategory::Food);
        self.add_emoji("🥝", "kiwi fruit", &["fruit", "kiwi"], EmojiCategory::Food);
        self.add_emoji("🍅", "tomato", &["vegetable"], EmojiCategory::Food);
        self.add_emoji("🥑", "avocado", &["guacamole"], EmojiCategory::Food);
        self.add_emoji("🥕", "carrot", &["vegetable"], EmojiCategory::Food);
        self.add_emoji("🌽", "ear of corn", &["corn", "vegetable"], EmojiCategory::Food);
        self.add_emoji("🥔", "potato", &["vegetable"], EmojiCategory::Food);
        self.add_emoji("🍞", "bread", &["toast", "loaf"], EmojiCategory::Food);
        self.add_emoji("🥐", "croissant", &["french", "breakfast"], EmojiCategory::Food);
        self.add_emoji("🥖", "baguette bread", &["french", "bread"], EmojiCategory::Food);
        self.add_emoji("🧀", "cheese wedge", &["cheese"], EmojiCategory::Food);
        self.add_emoji("🥚", "egg", &["breakfast"], EmojiCategory::Food);
        self.add_emoji("🍳", "cooking", &["egg", "breakfast", "fry"], EmojiCategory::Food);
        self.add_emoji("🥓", "bacon", &["breakfast", "meat"], EmojiCategory::Food);
        self.add_emoji("🥩", "cut of meat", &["steak", "meat"], EmojiCategory::Food);
        self.add_emoji("🍗", "poultry leg", &["chicken", "meat"], EmojiCategory::Food);
        self.add_emoji("🍖", "meat on bone", &["meat"], EmojiCategory::Food);
        self.add_emoji("🍔", "hamburger", &["burger", "fast food"], EmojiCategory::Food);
        self.add_emoji("🍟", "french fries", &["fries", "fast food"], EmojiCategory::Food);
        self.add_emoji("🍕", "pizza", &["italian", "fast food"], EmojiCategory::Food);
        self.add_emoji("🌭", "hot dog", &["fast food"], EmojiCategory::Food);
        self.add_emoji("🥪", "sandwich", &["lunch"], EmojiCategory::Food);
        self.add_emoji("🌮", "taco", &["mexican"], EmojiCategory::Food);
        self.add_emoji("🌯", "burrito", &["mexican"], EmojiCategory::Food);
        self.add_emoji("🍜", "steaming bowl", &["noodles", "ramen"], EmojiCategory::Food);
        self.add_emoji("🍝", "spaghetti", &["pasta", "italian"], EmojiCategory::Food);
        self.add_emoji("🍣", "sushi", &["japanese", "fish"], EmojiCategory::Food);
        self.add_emoji("🍱", "bento box", &["japanese", "lunch"], EmojiCategory::Food);
        self.add_emoji("🍩", "doughnut", &["donut", "dessert"], EmojiCategory::Food);
        self.add_emoji("🍪", "cookie", &["dessert", "biscuit"], EmojiCategory::Food);
        self.add_emoji("🎂", "birthday cake", &["cake", "birthday"], EmojiCategory::Food);
        self.add_emoji("🍰", "shortcake", &["cake", "dessert"], EmojiCategory::Food);
        self.add_emoji("🍦", "soft ice cream", &["icecream", "dessert"], EmojiCategory::Food);
        self.add_emoji("🍨", "ice cream", &["icecream", "dessert"], EmojiCategory::Food);
        self.add_emoji("🍫", "chocolate bar", &["chocolate", "candy"], EmojiCategory::Food);
        self.add_emoji("🍬", "candy", &["sweet"], EmojiCategory::Food);
        self.add_emoji("☕", "hot beverage", &["coffee", "tea"], EmojiCategory::Food);
        self.add_emoji("🍵", "teacup without handle", &["tea", "green tea"], EmojiCategory::Food);
        self.add_emoji("🍺", "beer mug", &["beer", "drink"], EmojiCategory::Food);
        self.add_emoji("🍻", "clinking beer mugs", &["beer", "cheers"], EmojiCategory::Food);
        self.add_emoji("🥂", "clinking glasses", &["champagne", "cheers"], EmojiCategory::Food);
        self.add_emoji("🍷", "wine glass", &["wine", "drink"], EmojiCategory::Food);
        self.add_emoji("🥤", "cup with straw", &["soda", "drink"], EmojiCategory::Food);

        // Travel & Places
        self.add_emoji("✈️", "airplane", &["plane", "travel", "flight"], EmojiCategory::Travel);
        self.add_emoji("🚗", "automobile", &["car", "drive"], EmojiCategory::Travel);
        self.add_emoji("🚕", "taxi", &["cab", "car"], EmojiCategory::Travel);
        self.add_emoji("🚌", "bus", &["transport"], EmojiCategory::Travel);
        self.add_emoji("🚎", "trolleybus", &["bus", "transport"], EmojiCategory::Travel);
        self.add_emoji("🚃", "railway car", &["train"], EmojiCategory::Travel);
        self.add_emoji("🚂", "locomotive", &["train"], EmojiCategory::Travel);
        self.add_emoji("🚆", "train", &["rail"], EmojiCategory::Travel);
        self.add_emoji("🚇", "metro", &["subway", "underground"], EmojiCategory::Travel);
        self.add_emoji("🚢", "ship", &["boat", "cruise"], EmojiCategory::Travel);
        self.add_emoji("⛵", "sailboat", &["boat", "sailing"], EmojiCategory::Travel);
        self.add_emoji("🚀", "rocket", &["space", "launch"], EmojiCategory::Travel);
        self.add_emoji("🛸", "flying saucer", &["ufo", "alien"], EmojiCategory::Travel);
        self.add_emoji("🚁", "helicopter", &["chopper"], EmojiCategory::Travel);
        self.add_emoji("🚲", "bicycle", &["bike", "cycling"], EmojiCategory::Travel);
        self.add_emoji("🏠", "house", &["home"], EmojiCategory::Travel);
        self.add_emoji("🏡", "house with garden", &["home"], EmojiCategory::Travel);
        self.add_emoji("🏢", "office building", &["work", "building"], EmojiCategory::Travel);
        self.add_emoji("🏥", "hospital", &["health", "medical"], EmojiCategory::Travel);
        self.add_emoji("🏦", "bank", &["money"], EmojiCategory::Travel);
        self.add_emoji("🏨", "hotel", &["accommodation"], EmojiCategory::Travel);
        self.add_emoji("🏪", "convenience store", &["shop"], EmojiCategory::Travel);
        self.add_emoji("🏫", "school", &["education"], EmojiCategory::Travel);
        self.add_emoji("⛪", "church", &["religion"], EmojiCategory::Travel);
        self.add_emoji("🗽", "Statue of Liberty", &["new york", "usa"], EmojiCategory::Travel);
        self.add_emoji("🗼", "Tokyo tower", &["japan", "tokyo"], EmojiCategory::Travel);
        self.add_emoji("🗻", "mount fuji", &["japan", "mountain"], EmojiCategory::Travel);
        self.add_emoji("🌋", "volcano", &["mountain"], EmojiCategory::Travel);
        self.add_emoji("🏝️", "desert island", &["beach", "vacation"], EmojiCategory::Travel);
        self.add_emoji("🏖️", "beach with umbrella", &["beach", "vacation"], EmojiCategory::Travel);
        self.add_emoji("🌅", "sunrise", &["morning", "sun"], EmojiCategory::Travel);
        self.add_emoji("🌄", "sunrise over mountains", &["morning", "sun"], EmojiCategory::Travel);
        self.add_emoji("🌃", "night with stars", &["night", "city"], EmojiCategory::Travel);
        self.add_emoji("🌉", "bridge at night", &["night", "city"], EmojiCategory::Travel);
        self.add_emoji("🌌", "milky way", &["space", "galaxy"], EmojiCategory::Travel);

        // Activities
        self.add_emoji("⚽", "soccer ball", &["football", "sport"], EmojiCategory::Activities);
        self.add_emoji("🏀", "basketball", &["sport", "ball"], EmojiCategory::Activities);
        self.add_emoji("🏈", "american football", &["sport", "nfl"], EmojiCategory::Activities);
        self.add_emoji("⚾", "baseball", &["sport"], EmojiCategory::Activities);
        self.add_emoji("🎾", "tennis", &["sport", "ball"], EmojiCategory::Activities);
        self.add_emoji("🏐", "volleyball", &["sport", "ball"], EmojiCategory::Activities);
        self.add_emoji("🏉", "rugby football", &["sport"], EmojiCategory::Activities);
        self.add_emoji("🎱", "pool 8 ball", &["billiards"], EmojiCategory::Activities);
        self.add_emoji("🏓", "ping pong", &["table tennis"], EmojiCategory::Activities);
        self.add_emoji("🏸", "badminton", &["sport"], EmojiCategory::Activities);
        self.add_emoji("🥊", "boxing glove", &["boxing", "fight"], EmojiCategory::Activities);
        self.add_emoji("🥋", "martial arts uniform", &["karate", "judo"], EmojiCategory::Activities);
        self.add_emoji("⛳", "flag in hole", &["golf"], EmojiCategory::Activities);
        self.add_emoji("🎿", "skis", &["skiing", "winter"], EmojiCategory::Activities);
        self.add_emoji("🏂", "snowboarder", &["snowboard", "winter"], EmojiCategory::Activities);
        self.add_emoji("🏋️", "person lifting weights", &["gym", "workout"], EmojiCategory::Activities);
        self.add_emoji("🤸", "person cartwheeling", &["gymnastics"], EmojiCategory::Activities);
        self.add_emoji("🏊", "person swimming", &["swim"], EmojiCategory::Activities);
        self.add_emoji("🚴", "person biking", &["cycling", "bike"], EmojiCategory::Activities);
        self.add_emoji("🧗", "person climbing", &["climbing", "rock"], EmojiCategory::Activities);
        self.add_emoji("🎮", "video game", &["gaming", "controller"], EmojiCategory::Activities);
        self.add_emoji("🎯", "direct hit", &["target", "bullseye"], EmojiCategory::Activities);
        self.add_emoji("🎲", "game die", &["dice", "gambling"], EmojiCategory::Activities);
        self.add_emoji("🎰", "slot machine", &["gambling", "casino"], EmojiCategory::Activities);
        self.add_emoji("🎳", "bowling", &["sport"], EmojiCategory::Activities);
        self.add_emoji("🎪", "circus tent", &["circus"], EmojiCategory::Activities);
        self.add_emoji("🎭", "performing arts", &["theater", "drama"], EmojiCategory::Activities);
        self.add_emoji("🎨", "artist palette", &["art", "painting"], EmojiCategory::Activities);
        self.add_emoji("🎬", "clapper board", &["movie", "film"], EmojiCategory::Activities);
        self.add_emoji("🎤", "microphone", &["karaoke", "sing"], EmojiCategory::Activities);
        self.add_emoji("🎧", "headphone", &["music", "audio"], EmojiCategory::Activities);
        self.add_emoji("🎼", "musical score", &["music"], EmojiCategory::Activities);
        self.add_emoji("🎹", "musical keyboard", &["piano", "music"], EmojiCategory::Activities);
        self.add_emoji("🎸", "guitar", &["music", "rock"], EmojiCategory::Activities);
        self.add_emoji("🎺", "trumpet", &["music", "jazz"], EmojiCategory::Activities);
        self.add_emoji("🎻", "violin", &["music", "classical"], EmojiCategory::Activities);
        self.add_emoji("🥁", "drum", &["music", "percussion"], EmojiCategory::Activities);
        self.add_emoji("🏆", "trophy", &["win", "award"], EmojiCategory::Activities);
        self.add_emoji("🥇", "1st place medal", &["gold", "first"], EmojiCategory::Activities);
        self.add_emoji("🥈", "2nd place medal", &["silver", "second"], EmojiCategory::Activities);
        self.add_emoji("🥉", "3rd place medal", &["bronze", "third"], EmojiCategory::Activities);

        // Objects
        self.add_emoji("⌚", "watch", &["time", "clock"], EmojiCategory::Objects);
        self.add_emoji("📱", "mobile phone", &["phone", "smartphone"], EmojiCategory::Objects);
        self.add_emoji("💻", "laptop", &["computer", "pc"], EmojiCategory::Objects);
        self.add_emoji("🖥️", "desktop computer", &["computer", "pc"], EmojiCategory::Objects);
        self.add_emoji("🖨️", "printer", &["print"], EmojiCategory::Objects);
        self.add_emoji("⌨️", "keyboard", &["type", "computer"], EmojiCategory::Objects);
        self.add_emoji("🖱️", "computer mouse", &["click"], EmojiCategory::Objects);
        self.add_emoji("💾", "floppy disk", &["save", "storage"], EmojiCategory::Objects);
        self.add_emoji("💿", "optical disk", &["cd", "dvd"], EmojiCategory::Objects);
        self.add_emoji("📷", "camera", &["photo"], EmojiCategory::Objects);
        self.add_emoji("📹", "video camera", &["video", "record"], EmojiCategory::Objects);
        self.add_emoji("🎥", "movie camera", &["film", "cinema"], EmojiCategory::Objects);
        self.add_emoji("📺", "television", &["tv"], EmojiCategory::Objects);
        self.add_emoji("📻", "radio", &["audio"], EmojiCategory::Objects);
        self.add_emoji("🔦", "flashlight", &["light", "torch"], EmojiCategory::Objects);
        self.add_emoji("💡", "light bulb", &["idea", "light"], EmojiCategory::Objects);
        self.add_emoji("🔌", "electric plug", &["power"], EmojiCategory::Objects);
        self.add_emoji("🔋", "battery", &["power", "energy"], EmojiCategory::Objects);
        self.add_emoji("🔧", "wrench", &["tool", "fix"], EmojiCategory::Objects);
        self.add_emoji("🔨", "hammer", &["tool", "build"], EmojiCategory::Objects);
        self.add_emoji("🔩", "nut and bolt", &["tool"], EmojiCategory::Objects);
        self.add_emoji("⚙️", "gear", &["settings", "cog"], EmojiCategory::Objects);
        self.add_emoji("🔗", "link", &["chain", "url"], EmojiCategory::Objects);
        self.add_emoji("📎", "paperclip", &["attach"], EmojiCategory::Objects);
        self.add_emoji("✂️", "scissors", &["cut"], EmojiCategory::Objects);
        self.add_emoji("📝", "memo", &["note", "write"], EmojiCategory::Objects);
        self.add_emoji("✏️", "pencil", &["write", "edit"], EmojiCategory::Objects);
        self.add_emoji("📏", "straight ruler", &["measure"], EmojiCategory::Objects);
        self.add_emoji("📐", "triangular ruler", &["measure"], EmojiCategory::Objects);
        self.add_emoji("📚", "books", &["read", "library"], EmojiCategory::Objects);
        self.add_emoji("📖", "open book", &["read"], EmojiCategory::Objects);
        self.add_emoji("📰", "newspaper", &["news"], EmojiCategory::Objects);
        self.add_emoji("📧", "e-mail", &["email", "mail"], EmojiCategory::Objects);
        self.add_emoji("📦", "package", &["box", "shipping"], EmojiCategory::Objects);
        self.add_emoji("🔒", "locked", &["security", "lock"], EmojiCategory::Objects);
        self.add_emoji("🔓", "unlocked", &["open", "lock"], EmojiCategory::Objects);
        self.add_emoji("🔑", "key", &["unlock", "password"], EmojiCategory::Objects);
        self.add_emoji("💰", "money bag", &["money", "dollar"], EmojiCategory::Objects);
        self.add_emoji("💳", "credit card", &["payment", "money"], EmojiCategory::Objects);
        self.add_emoji("💎", "gem stone", &["diamond", "jewel"], EmojiCategory::Objects);
        self.add_emoji("⏰", "alarm clock", &["time", "wake"], EmojiCategory::Objects);
        self.add_emoji("⏳", "hourglass not done", &["time", "wait"], EmojiCategory::Objects);

        // Symbols
        self.add_emoji("❤️", "red heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("🧡", "orange heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("💛", "yellow heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("💚", "green heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("💙", "blue heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("💜", "purple heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("🖤", "black heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("🤍", "white heart", &["love", "heart"], EmojiCategory::Symbols);
        self.add_emoji("💔", "broken heart", &["heartbreak", "sad"], EmojiCategory::Symbols);
        self.add_emoji("💕", "two hearts", &["love"], EmojiCategory::Symbols);
        self.add_emoji("💖", "sparkling heart", &["love"], EmojiCategory::Symbols);
        self.add_emoji("💗", "growing heart", &["love"], EmojiCategory::Symbols);
        self.add_emoji("💘", "heart with arrow", &["cupid", "love"], EmojiCategory::Symbols);
        self.add_emoji("💝", "heart with ribbon", &["love", "gift"], EmojiCategory::Symbols);
        self.add_emoji("✅", "check mark button", &["yes", "done"], EmojiCategory::Symbols);
        self.add_emoji("❌", "cross mark", &["no", "wrong"], EmojiCategory::Symbols);
        self.add_emoji("❓", "question mark", &["question"], EmojiCategory::Symbols);
        self.add_emoji("❗", "exclamation mark", &["important"], EmojiCategory::Symbols);
        self.add_emoji("⭐", "star", &["favorite"], EmojiCategory::Symbols);
        self.add_emoji("🌟", "glowing star", &["shine"], EmojiCategory::Symbols);
        self.add_emoji("✨", "sparkles", &["magic", "shine"], EmojiCategory::Symbols);
        self.add_emoji("💫", "dizzy", &["star"], EmojiCategory::Symbols);
        self.add_emoji("💥", "collision", &["boom", "explosion"], EmojiCategory::Symbols);
        self.add_emoji("💢", "anger symbol", &["angry"], EmojiCategory::Symbols);
        self.add_emoji("💤", "zzz", &["sleep"], EmojiCategory::Symbols);
        self.add_emoji("💬", "speech balloon", &["chat", "message"], EmojiCategory::Symbols);
        self.add_emoji("💭", "thought balloon", &["think"], EmojiCategory::Symbols);
        self.add_emoji("🔔", "bell", &["notification", "alert"], EmojiCategory::Symbols);
        self.add_emoji("🔕", "bell with slash", &["mute", "silent"], EmojiCategory::Symbols);
        self.add_emoji("🎵", "musical note", &["music"], EmojiCategory::Symbols);
        self.add_emoji("🎶", "musical notes", &["music"], EmojiCategory::Symbols);
        self.add_emoji("➕", "plus", &["add"], EmojiCategory::Symbols);
        self.add_emoji("➖", "minus", &["subtract"], EmojiCategory::Symbols);
        self.add_emoji("➗", "divide", &["division"], EmojiCategory::Symbols);
        self.add_emoji("✖️", "multiply", &["times"], EmojiCategory::Symbols);
        self.add_emoji("♻️", "recycling symbol", &["recycle", "environment"], EmojiCategory::Symbols);
        self.add_emoji("⚠️", "warning", &["caution", "alert"], EmojiCategory::Symbols);
        self.add_emoji("🚫", "prohibited", &["no", "forbidden"], EmojiCategory::Symbols);
        self.add_emoji("🔴", "red circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("🟠", "orange circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("🟡", "yellow circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("🟢", "green circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("🔵", "blue circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("🟣", "purple circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("⚫", "black circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("⚪", "white circle", &["circle"], EmojiCategory::Symbols);
        self.add_emoji("🔶", "large orange diamond", &["diamond"], EmojiCategory::Symbols);
        self.add_emoji("🔷", "large blue diamond", &["diamond"], EmojiCategory::Symbols);

        // Flags
        self.add_emoji("🏳️", "white flag", &["surrender"], EmojiCategory::Flags);
        self.add_emoji("🏴", "black flag", &["flag"], EmojiCategory::Flags);
        self.add_emoji("🏁", "chequered flag", &["race", "finish"], EmojiCategory::Flags);
        self.add_emoji("🚩", "triangular flag", &["flag"], EmojiCategory::Flags);
        self.add_emoji("🇺🇸", "flag: United States", &["usa", "america"], EmojiCategory::Flags);
        self.add_emoji("🇬🇧", "flag: United Kingdom", &["uk", "britain"], EmojiCategory::Flags);
        self.add_emoji("🇨🇦", "flag: Canada", &["canada"], EmojiCategory::Flags);
        self.add_emoji("🇦🇺", "flag: Australia", &["australia"], EmojiCategory::Flags);
        self.add_emoji("🇩🇪", "flag: Germany", &["germany"], EmojiCategory::Flags);
        self.add_emoji("🇫🇷", "flag: France", &["france"], EmojiCategory::Flags);
        self.add_emoji("🇮🇹", "flag: Italy", &["italy"], EmojiCategory::Flags);
        self.add_emoji("🇪🇸", "flag: Spain", &["spain"], EmojiCategory::Flags);
        self.add_emoji("🇵🇹", "flag: Portugal", &["portugal"], EmojiCategory::Flags);
        self.add_emoji("🇧🇷", "flag: Brazil", &["brazil"], EmojiCategory::Flags);
        self.add_emoji("🇲🇽", "flag: Mexico", &["mexico"], EmojiCategory::Flags);
        self.add_emoji("🇯🇵", "flag: Japan", &["japan"], EmojiCategory::Flags);
        self.add_emoji("🇰🇷", "flag: South Korea", &["korea"], EmojiCategory::Flags);
        self.add_emoji("🇨🇳", "flag: China", &["china"], EmojiCategory::Flags);
        self.add_emoji("🇮🇳", "flag: India", &["india"], EmojiCategory::Flags);
        self.add_emoji("🇷🇺", "flag: Russia", &["russia"], EmojiCategory::Flags);

        // Build category map
        for (i, emoji) in self.emojis.iter().enumerate() {
            let cat_key = emoji.category as u8;
            self.category_map.entry(cat_key).or_insert_with(Vec::new).push(i);
        }
    }

    fn add_emoji(&mut self, emoji: &'static str, name: &'static str, keywords: &'static [&'static str], category: EmojiCategory) {
        self.emojis.push(Emoji::new(emoji, name, keywords, category));
    }

    /// Open the picker
    pub fn open(&mut self) {
        self.state = PickerState::Browse;
        self.selected_index = 0;
    }

    /// Close the picker
    pub fn close(&mut self) {
        self.state = PickerState::Closed;
        self.search_query.clear();
        self.search_results.clear();
    }

    /// Toggle picker
    pub fn toggle(&mut self) {
        if self.state == PickerState::Closed {
            self.open();
        } else {
            self.close();
        }
    }

    /// Check if picker is open
    pub fn is_open(&self) -> bool {
        self.state != PickerState::Closed
    }

    /// Get current state
    pub fn state(&self) -> PickerState {
        self.state
    }

    /// Start search mode
    pub fn start_search(&mut self) {
        self.state = PickerState::Search;
        self.search_query.clear();
        self.search_results.clear();
        self.selected_index = 0;
    }

    /// Add character to search query
    pub fn search_input(&mut self, ch: char) {
        self.search_query.push(ch);
        self.update_search();
    }

    /// Remove last character from search
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.update_search();
    }

    /// Update search results
    fn update_search(&mut self) {
        self.search_results.clear();
        if self.search_query.is_empty() {
            return;
        }

        for (i, emoji) in self.emojis.iter().enumerate() {
            if emoji.matches(&self.search_query) {
                self.search_results.push(i);
            }
        }
        self.selected_index = 0;
    }

    /// Get search query
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Set current category
    pub fn set_category(&mut self, category: EmojiCategory) {
        self.current_category = category;
        self.state = PickerState::Browse;
        self.selected_index = 0;
    }

    /// Get current category
    pub fn category(&self) -> EmojiCategory {
        self.current_category
    }

    /// Get emojis for current view
    pub fn current_emojis(&self) -> Vec<&Emoji> {
        match self.state {
            PickerState::Search => {
                self.search_results.iter()
                    .filter_map(|&i| self.emojis.get(i))
                    .collect()
            }
            PickerState::Browse => {
                if self.current_category == EmojiCategory::Recent {
                    self.recent.iter()
                        .filter_map(|&i| self.emojis.get(i))
                        .collect()
                } else {
                    let cat_key = self.current_category as u8;
                    self.category_map.get(&cat_key)
                        .map(|indices| {
                            indices.iter()
                                .filter_map(|&i| self.emojis.get(i))
                                .collect()
                        })
                        .unwrap_or_default()
                }
            }
            PickerState::Closed => Vec::new(),
        }
    }

    /// Move selection
    pub fn move_selection(&mut self, delta: i32) {
        let count = self.current_emojis().len();
        if count == 0 {
            return;
        }

        let current = self.selected_index as i32;
        let new_index = (current + delta).rem_euclid(count as i32) as usize;
        self.selected_index = new_index;
    }

    /// Move selection up (previous row)
    pub fn move_up(&mut self) {
        self.move_selection(-(self.config.columns as i32));
    }

    /// Move selection down (next row)
    pub fn move_down(&mut self) {
        self.move_selection(self.config.columns as i32);
    }

    /// Move selection left
    pub fn move_left(&mut self) {
        self.move_selection(-1);
    }

    /// Move selection right
    pub fn move_right(&mut self) {
        self.move_selection(1);
    }

    /// Get selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Select emoji and return it
    pub fn select(&mut self) -> Option<String> {
        let emojis = self.current_emojis();
        if let Some(emoji) = emojis.get(self.selected_index) {
            let result = emoji.emoji.to_string();

            // Find index in main list and add to recent
            if let Some(idx) = self.emojis.iter().position(|e| e.emoji == emoji.emoji) {
                self.add_to_recent(idx);
            }

            Some(result)
        } else {
            None
        }
    }

    /// Add emoji to recent list
    fn add_to_recent(&mut self, index: usize) {
        // Remove if already in recent
        self.recent.retain(|&i| i != index);

        // Add to front
        self.recent.insert(0, index);

        // Trim to max
        if self.recent.len() > self.config.max_recent {
            self.recent.truncate(self.config.max_recent);
        }
    }

    /// Get emoji by index from database
    pub fn get_emoji(&self, index: usize) -> Option<&Emoji> {
        self.emojis.get(index)
    }

    /// Total emoji count
    pub fn emoji_count(&self) -> usize {
        self.emojis.len()
    }

    /// Get config
    pub fn config(&self) -> &EmojiPickerConfig {
        &self.config
    }

    /// Set config
    pub fn set_config(&mut self, config: EmojiPickerConfig) {
        self.config = config;
    }

    /// Set skin tone
    pub fn set_skin_tone(&mut self, tone: SkinTone) {
        self.config.skin_tone = tone;
    }
}

impl Default for EmojiPicker {
    fn default() -> Self {
        Self::new()
    }
}

// Global emoji picker instance
use crate::sync::IrqSafeMutex;

static EMOJI_PICKER: IrqSafeMutex<Option<EmojiPicker>> = IrqSafeMutex::new(None);

/// Initialize emoji picker
pub fn init() {
    let mut picker = EMOJI_PICKER.lock();
    *picker = Some(EmojiPicker::new());
}

/// Open picker
pub fn open() {
    if let Some(ref mut picker) = *EMOJI_PICKER.lock() {
        picker.open();
    }
}

/// Close picker
pub fn close() {
    if let Some(ref mut picker) = *EMOJI_PICKER.lock() {
        picker.close();
    }
}

/// Toggle picker
pub fn toggle() {
    if let Some(ref mut picker) = *EMOJI_PICKER.lock() {
        picker.toggle();
    }
}

/// Check if open
pub fn is_open() -> bool {
    EMOJI_PICKER.lock().as_ref().map(|p| p.is_open()).unwrap_or(false)
}

/// Select current emoji
pub fn select() -> Option<String> {
    EMOJI_PICKER.lock().as_mut().and_then(|p| p.select())
}
