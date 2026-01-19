//! Chinese Pinyin Input Method Engine
//!
//! Provides Pinyin-based Chinese input with candidate selection.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::vec;

use super::ibus::{
    InputMethodEngine, InputMethodType, InputMethodState,
    Candidate, InputEvent, InputResult,
};

/// Pinyin engine configuration
#[derive(Debug, Clone)]
pub struct PinyinConfig {
    /// Enable fuzzy matching
    pub fuzzy_pinyin: bool,
    /// Enable simplified Chinese
    pub simplified: bool,
    /// Max candidates to show
    pub max_candidates: usize,
    /// Enable cloud suggestions (placeholder)
    pub cloud_suggestions: bool,
}

impl Default for PinyinConfig {
    fn default() -> Self {
        Self {
            fuzzy_pinyin: true,
            simplified: true,
            max_candidates: 9,
            cloud_suggestions: false,
        }
    }
}

/// Pinyin input engine
pub struct PinyinEngine {
    /// Configuration
    config: PinyinConfig,
    /// Current state
    state: InputMethodState,
    /// Current pinyin buffer
    pinyin_buffer: String,
    /// Current candidates
    candidates: Vec<Candidate>,
    /// Selected candidate index
    selected: usize,
    /// Pinyin dictionary (syllable -> characters)
    dictionary: BTreeMap<String, Vec<String>>,
}

impl PinyinEngine {
    /// Create a new Pinyin engine
    pub fn new() -> Self {
        let mut engine = Self {
            config: PinyinConfig::default(),
            state: InputMethodState::Idle,
            pinyin_buffer: String::new(),
            candidates: Vec::new(),
            selected: 0,
            dictionary: BTreeMap::new(),
        };
        engine.load_dictionary();
        engine
    }

    /// Load basic pinyin dictionary
    fn load_dictionary(&mut self) {
        // Common pinyin syllables with their most frequent characters
        // This is a minimal dictionary - a real implementation would have thousands of entries
        self.add_pinyin("a", &["啊", "阿", "呵"]);
        self.add_pinyin("ai", &["爱", "哀", "唉", "矮", "癌"]);
        self.add_pinyin("an", &["安", "按", "暗", "岸", "案"]);
        self.add_pinyin("ang", &["昂"]);
        self.add_pinyin("ao", &["奥", "傲", "熬"]);

        self.add_pinyin("ba", &["八", "把", "吧", "爸", "霸"]);
        self.add_pinyin("bai", &["白", "百", "拜", "败"]);
        self.add_pinyin("ban", &["半", "办", "班", "般", "版"]);
        self.add_pinyin("bang", &["帮", "棒", "榜"]);
        self.add_pinyin("bao", &["包", "报", "保", "抱", "宝"]);
        self.add_pinyin("bei", &["北", "杯", "背", "被", "悲"]);
        self.add_pinyin("ben", &["本", "奔", "笨"]);
        self.add_pinyin("bi", &["比", "笔", "必", "闭", "避"]);
        self.add_pinyin("bian", &["边", "变", "便", "遍", "编"]);
        self.add_pinyin("biao", &["表", "标", "彪"]);
        self.add_pinyin("bie", &["别", "憋"]);
        self.add_pinyin("bin", &["宾", "滨"]);
        self.add_pinyin("bing", &["冰", "病", "并", "兵"]);
        self.add_pinyin("bo", &["波", "播", "博", "薄"]);
        self.add_pinyin("bu", &["不", "步", "部", "布"]);

        self.add_pinyin("ca", &["擦"]);
        self.add_pinyin("cai", &["才", "菜", "采", "财", "猜"]);
        self.add_pinyin("can", &["参", "惨", "餐", "残"]);
        self.add_pinyin("cang", &["藏", "仓", "苍"]);
        self.add_pinyin("cao", &["草", "操", "糙"]);
        self.add_pinyin("ce", &["测", "策", "侧", "厕"]);
        self.add_pinyin("ceng", &["层", "曾"]);
        self.add_pinyin("cha", &["查", "茶", "差", "插"]);
        self.add_pinyin("chai", &["拆", "柴"]);
        self.add_pinyin("chan", &["产", "缠", "颤"]);
        self.add_pinyin("chang", &["长", "常", "场", "唱", "厂"]);
        self.add_pinyin("chao", &["超", "潮", "朝", "吵", "炒"]);
        self.add_pinyin("che", &["车", "彻", "扯"]);
        self.add_pinyin("chen", &["陈", "沉", "晨", "尘", "称"]);
        self.add_pinyin("cheng", &["成", "城", "程", "称", "乘"]);
        self.add_pinyin("chi", &["吃", "池", "迟", "持", "尺"]);
        self.add_pinyin("chong", &["充", "冲", "虫", "重"]);
        self.add_pinyin("chou", &["抽", "愁", "丑", "臭"]);
        self.add_pinyin("chu", &["出", "处", "初", "除", "楚"]);
        self.add_pinyin("chuang", &["创", "窗", "床", "闯"]);
        self.add_pinyin("chui", &["吹", "垂", "锤"]);
        self.add_pinyin("chun", &["春", "纯", "蠢"]);
        self.add_pinyin("ci", &["词", "此", "次", "刺", "磁"]);
        self.add_pinyin("cong", &["从", "聪", "葱"]);
        self.add_pinyin("cu", &["粗", "醋", "促"]);
        self.add_pinyin("cuan", &["窜"]);
        self.add_pinyin("cui", &["催", "脆"]);
        self.add_pinyin("cun", &["村", "存", "寸"]);
        self.add_pinyin("cuo", &["错", "措"]);

        self.add_pinyin("da", &["大", "打", "达", "答"]);
        self.add_pinyin("dai", &["带", "代", "待", "袋", "戴"]);
        self.add_pinyin("dan", &["但", "单", "担", "蛋", "胆"]);
        self.add_pinyin("dang", &["当", "党", "挡", "档"]);
        self.add_pinyin("dao", &["到", "道", "刀", "倒", "导"]);
        self.add_pinyin("de", &["的", "得", "德"]);
        self.add_pinyin("deng", &["等", "灯", "登"]);
        self.add_pinyin("di", &["地", "第", "低", "底", "弟"]);
        self.add_pinyin("dian", &["点", "电", "店", "典"]);
        self.add_pinyin("diao", &["掉", "调", "钓", "雕"]);
        self.add_pinyin("die", &["跌", "爹", "蝶"]);
        self.add_pinyin("ding", &["定", "顶", "订", "钉"]);
        self.add_pinyin("dong", &["东", "动", "冬", "懂", "洞"]);
        self.add_pinyin("dou", &["都", "斗", "豆", "逗"]);
        self.add_pinyin("du", &["读", "度", "毒", "独", "堵"]);
        self.add_pinyin("duan", &["段", "短", "断"]);
        self.add_pinyin("dui", &["对", "队", "堆"]);
        self.add_pinyin("dun", &["顿", "蹲", "盾"]);
        self.add_pinyin("duo", &["多", "夺", "朵", "躲"]);

        self.add_pinyin("e", &["饿", "恶", "额", "俄"]);
        self.add_pinyin("ei", &["诶"]);
        self.add_pinyin("en", &["恩", "嗯"]);
        self.add_pinyin("er", &["二", "而", "儿", "耳"]);

        self.add_pinyin("fa", &["发", "法", "罚", "乏"]);
        self.add_pinyin("fan", &["反", "饭", "烦", "犯", "番"]);
        self.add_pinyin("fang", &["方", "房", "放", "防", "访"]);
        self.add_pinyin("fei", &["飞", "非", "费", "肥", "废"]);
        self.add_pinyin("fen", &["分", "粉", "份", "愤", "纷"]);
        self.add_pinyin("feng", &["风", "封", "峰", "丰", "锋"]);
        self.add_pinyin("fo", &["佛"]);
        self.add_pinyin("fou", &["否"]);
        self.add_pinyin("fu", &["服", "父", "夫", "付", "复"]);

        self.add_pinyin("gai", &["该", "改", "概", "盖"]);
        self.add_pinyin("gan", &["干", "感", "敢", "赶", "甘"]);
        self.add_pinyin("gang", &["刚", "钢", "港", "纲"]);
        self.add_pinyin("gao", &["高", "搞", "告", "稿"]);
        self.add_pinyin("ge", &["个", "各", "歌", "哥", "格"]);
        self.add_pinyin("gei", &["给"]);
        self.add_pinyin("gen", &["根", "跟"]);
        self.add_pinyin("geng", &["更", "耕"]);
        self.add_pinyin("gong", &["工", "公", "共", "功", "供"]);
        self.add_pinyin("gou", &["够", "狗", "购", "沟", "勾"]);
        self.add_pinyin("gu", &["古", "故", "骨", "谷", "股"]);
        self.add_pinyin("gua", &["瓜", "挂", "刮"]);
        self.add_pinyin("guai", &["怪", "乖", "拐"]);
        self.add_pinyin("guan", &["关", "管", "观", "官", "惯"]);
        self.add_pinyin("guang", &["光", "广", "逛"]);
        self.add_pinyin("gui", &["贵", "鬼", "归", "规"]);
        self.add_pinyin("gun", &["滚", "棍"]);
        self.add_pinyin("guo", &["国", "过", "果", "锅", "郭"]);

        self.add_pinyin("ha", &["哈"]);
        self.add_pinyin("hai", &["还", "海", "害", "孩"]);
        self.add_pinyin("han", &["汉", "喊", "含", "寒", "韩"]);
        self.add_pinyin("hang", &["行", "航"]);
        self.add_pinyin("hao", &["好", "号", "毫", "豪", "耗"]);
        self.add_pinyin("he", &["和", "何", "河", "喝", "合"]);
        self.add_pinyin("hei", &["黑", "嘿"]);
        self.add_pinyin("hen", &["很", "恨", "狠"]);
        self.add_pinyin("heng", &["横", "哼"]);
        self.add_pinyin("hong", &["红", "宏", "洪", "轰"]);
        self.add_pinyin("hou", &["后", "候", "厚", "猴", "吼"]);
        self.add_pinyin("hu", &["湖", "虎", "户", "护", "呼"]);
        self.add_pinyin("hua", &["话", "花", "画", "化", "华"]);
        self.add_pinyin("huai", &["坏", "怀"]);
        self.add_pinyin("huan", &["还", "换", "欢", "环", "缓"]);
        self.add_pinyin("huang", &["黄", "皇", "慌", "荒"]);
        self.add_pinyin("hui", &["会", "回", "灰", "汇", "挥"]);
        self.add_pinyin("hun", &["混", "婚", "昏", "魂"]);
        self.add_pinyin("huo", &["活", "火", "或", "获", "货"]);

        self.add_pinyin("ji", &["几", "机", "基", "及", "己"]);
        self.add_pinyin("jia", &["家", "加", "价", "假", "嫁"]);
        self.add_pinyin("jian", &["间", "见", "建", "件", "简"]);
        self.add_pinyin("jiang", &["将", "讲", "江", "降", "奖"]);
        self.add_pinyin("jiao", &["叫", "教", "交", "脚", "角"]);
        self.add_pinyin("jie", &["接", "结", "街", "节", "解"]);
        self.add_pinyin("jin", &["进", "近", "金", "今", "尽"]);
        self.add_pinyin("jing", &["经", "精", "静", "京", "惊"]);
        self.add_pinyin("jiu", &["就", "九", "久", "酒", "旧"]);
        self.add_pinyin("ju", &["举", "句", "具", "据", "居"]);
        self.add_pinyin("juan", &["卷", "倦", "绢"]);
        self.add_pinyin("jue", &["觉", "决", "绝", "角"]);
        self.add_pinyin("jun", &["军", "均", "君", "菌"]);

        self.add_pinyin("ka", &["卡", "咖"]);
        self.add_pinyin("kai", &["开", "凯"]);
        self.add_pinyin("kan", &["看", "砍", "刊"]);
        self.add_pinyin("kang", &["扛", "抗", "康"]);
        self.add_pinyin("kao", &["考", "靠", "烤"]);
        self.add_pinyin("ke", &["可", "课", "科", "客", "刻"]);
        self.add_pinyin("ken", &["肯", "啃"]);
        self.add_pinyin("keng", &["坑"]);
        self.add_pinyin("kong", &["空", "控", "孔", "恐"]);
        self.add_pinyin("kou", &["口", "扣"]);
        self.add_pinyin("ku", &["苦", "哭", "库", "酷"]);
        self.add_pinyin("kua", &["夸", "跨"]);
        self.add_pinyin("kuai", &["快", "块", "筷"]);
        self.add_pinyin("kuan", &["宽", "款"]);
        self.add_pinyin("kuang", &["狂", "况", "矿", "框"]);
        self.add_pinyin("kun", &["困", "昆"]);
        self.add_pinyin("kuo", &["扩", "阔"]);

        self.add_pinyin("la", &["拉", "啦", "辣"]);
        self.add_pinyin("lai", &["来", "赖"]);
        self.add_pinyin("lan", &["蓝", "烂", "懒", "兰"]);
        self.add_pinyin("lang", &["浪", "狼", "郎"]);
        self.add_pinyin("lao", &["老", "劳", "牢"]);
        self.add_pinyin("le", &["了", "乐", "勒"]);
        self.add_pinyin("lei", &["类", "累", "泪", "雷"]);
        self.add_pinyin("leng", &["冷", "愣"]);
        self.add_pinyin("li", &["里", "理", "力", "离", "利"]);
        self.add_pinyin("lia", &["俩"]);
        self.add_pinyin("lian", &["连", "脸", "练", "联", "恋"]);
        self.add_pinyin("liang", &["两", "量", "亮", "凉", "粮"]);
        self.add_pinyin("liao", &["了", "料", "聊", "疗"]);
        self.add_pinyin("lie", &["列", "裂", "烈", "猎"]);
        self.add_pinyin("lin", &["林", "临", "邻"]);
        self.add_pinyin("ling", &["零", "领", "另", "灵", "铃"]);
        self.add_pinyin("liu", &["六", "留", "流", "刘"]);
        self.add_pinyin("long", &["龙", "隆", "弄"]);
        self.add_pinyin("lou", &["楼", "漏"]);
        self.add_pinyin("lu", &["路", "录", "绿", "露", "陆"]);
        self.add_pinyin("lv", &["绿", "旅", "律", "滤"]);
        self.add_pinyin("luan", &["乱", "卵"]);
        self.add_pinyin("lun", &["论", "轮", "伦"]);
        self.add_pinyin("luo", &["落", "罗", "络"]);

        self.add_pinyin("ma", &["吗", "妈", "马", "骂", "码"]);
        self.add_pinyin("mai", &["买", "卖", "迈", "埋"]);
        self.add_pinyin("man", &["满", "慢", "漫", "蛮"]);
        self.add_pinyin("mang", &["忙", "盲", "茫"]);
        self.add_pinyin("mao", &["毛", "猫", "帽", "冒", "贸"]);
        self.add_pinyin("me", &["么"]);
        self.add_pinyin("mei", &["没", "每", "美", "妹", "梅"]);
        self.add_pinyin("men", &["门", "们", "闷"]);
        self.add_pinyin("meng", &["梦", "蒙", "猛"]);
        self.add_pinyin("mi", &["米", "密", "迷", "蜜"]);
        self.add_pinyin("mian", &["面", "免", "棉", "眠"]);
        self.add_pinyin("miao", &["秒", "妙", "苗", "描"]);
        self.add_pinyin("mie", &["灭"]);
        self.add_pinyin("min", &["民", "敏", "闽"]);
        self.add_pinyin("ming", &["名", "明", "命", "鸣"]);
        self.add_pinyin("mo", &["没", "磨", "摸", "莫", "末"]);
        self.add_pinyin("mou", &["某", "谋"]);
        self.add_pinyin("mu", &["母", "木", "目", "墓", "幕"]);

        self.add_pinyin("na", &["那", "拿", "哪", "纳"]);
        self.add_pinyin("nai", &["奶", "乃", "耐"]);
        self.add_pinyin("nan", &["南", "男", "难"]);
        self.add_pinyin("nao", &["脑", "闹"]);
        self.add_pinyin("ne", &["呢"]);
        self.add_pinyin("nei", &["内", "那"]);
        self.add_pinyin("nen", &["嫩"]);
        self.add_pinyin("neng", &["能"]);
        self.add_pinyin("ni", &["你", "泥", "逆", "拟"]);
        self.add_pinyin("nian", &["年", "念", "粘"]);
        self.add_pinyin("niang", &["娘"]);
        self.add_pinyin("niao", &["鸟", "尿"]);
        self.add_pinyin("nin", &["您"]);
        self.add_pinyin("ning", &["宁", "凝"]);
        self.add_pinyin("niu", &["牛", "扭", "纽"]);
        self.add_pinyin("nong", &["农", "浓"]);
        self.add_pinyin("nu", &["女", "怒", "努"]);
        self.add_pinyin("nuan", &["暖"]);
        self.add_pinyin("nuo", &["诺"]);

        self.add_pinyin("o", &["哦", "噢"]);
        self.add_pinyin("ou", &["偶", "欧"]);

        self.add_pinyin("pa", &["怕", "拍", "爬", "帕"]);
        self.add_pinyin("pai", &["排", "派", "拍"]);
        self.add_pinyin("pan", &["盘", "判", "盼"]);
        self.add_pinyin("pang", &["旁", "胖"]);
        self.add_pinyin("pao", &["跑", "泡", "炮", "抛"]);
        self.add_pinyin("pei", &["配", "陪", "培", "赔"]);
        self.add_pinyin("pen", &["喷", "盆"]);
        self.add_pinyin("peng", &["朋", "碰", "棚", "捧"]);
        self.add_pinyin("pi", &["皮", "批", "匹", "劈", "屁"]);
        self.add_pinyin("pian", &["片", "骗", "便", "篇"]);
        self.add_pinyin("piao", &["票", "飘", "漂"]);
        self.add_pinyin("pin", &["品", "拼", "贫"]);
        self.add_pinyin("ping", &["平", "评", "瓶", "苹"]);
        self.add_pinyin("po", &["破", "坡", "泼"]);
        self.add_pinyin("pu", &["普", "铺", "扑"]);

        self.add_pinyin("qi", &["起", "其", "气", "七", "期"]);
        self.add_pinyin("qia", &["恰", "卡"]);
        self.add_pinyin("qian", &["前", "钱", "千", "签", "浅"]);
        self.add_pinyin("qiang", &["强", "墙", "抢", "枪"]);
        self.add_pinyin("qiao", &["桥", "敲", "巧", "瞧"]);
        self.add_pinyin("qie", &["切", "且", "窃"]);
        self.add_pinyin("qin", &["亲", "琴", "勤", "侵", "秦"]);
        self.add_pinyin("qing", &["请", "清", "情", "青", "轻"]);
        self.add_pinyin("qiu", &["球", "秋", "求"]);
        self.add_pinyin("qu", &["去", "取", "曲", "区"]);
        self.add_pinyin("quan", &["全", "权", "泉", "圈", "劝"]);
        self.add_pinyin("que", &["却", "确", "缺"]);
        self.add_pinyin("qun", &["群", "裙"]);

        self.add_pinyin("ran", &["然", "染", "燃"]);
        self.add_pinyin("rang", &["让", "嚷"]);
        self.add_pinyin("rao", &["绕", "扰"]);
        self.add_pinyin("re", &["热", "惹"]);
        self.add_pinyin("ren", &["人", "认", "任", "忍", "仁"]);
        self.add_pinyin("reng", &["仍", "扔"]);
        self.add_pinyin("ri", &["日"]);
        self.add_pinyin("rong", &["容", "融", "荣"]);
        self.add_pinyin("rou", &["肉", "柔"]);
        self.add_pinyin("ru", &["如", "入", "乳"]);
        self.add_pinyin("ruan", &["软"]);
        self.add_pinyin("rui", &["瑞", "锐"]);
        self.add_pinyin("run", &["润"]);
        self.add_pinyin("ruo", &["若", "弱"]);

        self.add_pinyin("sa", &["撒", "洒"]);
        self.add_pinyin("sai", &["赛", "塞"]);
        self.add_pinyin("san", &["三", "散", "伞"]);
        self.add_pinyin("sang", &["桑", "丧"]);
        self.add_pinyin("sao", &["扫", "骚"]);
        self.add_pinyin("se", &["色", "涩"]);
        self.add_pinyin("sen", &["森"]);
        self.add_pinyin("sha", &["杀", "沙", "傻", "纱"]);
        self.add_pinyin("shai", &["晒"]);
        self.add_pinyin("shan", &["山", "闪", "善", "扇"]);
        self.add_pinyin("shang", &["上", "伤", "商", "尚", "赏"]);
        self.add_pinyin("shao", &["少", "烧", "绍", "稍"]);
        self.add_pinyin("she", &["社", "设", "舍", "射", "蛇"]);
        self.add_pinyin("shei", &["谁"]);
        self.add_pinyin("shen", &["什", "深", "身", "神", "甚"]);
        self.add_pinyin("sheng", &["生", "声", "省", "胜", "升"]);
        self.add_pinyin("shi", &["是", "时", "事", "十", "使"]);
        self.add_pinyin("shou", &["手", "收", "受", "首", "守"]);
        self.add_pinyin("shu", &["书", "树", "数", "输", "属"]);
        self.add_pinyin("shua", &["刷", "耍"]);
        self.add_pinyin("shuai", &["帅", "摔", "衰"]);
        self.add_pinyin("shuan", &["栓"]);
        self.add_pinyin("shuang", &["双", "霜", "爽"]);
        self.add_pinyin("shui", &["水", "睡", "谁"]);
        self.add_pinyin("shun", &["顺", "瞬"]);
        self.add_pinyin("shuo", &["说", "硕"]);
        self.add_pinyin("si", &["四", "死", "思", "私", "司"]);
        self.add_pinyin("song", &["送", "松", "宋"]);
        self.add_pinyin("sou", &["搜"]);
        self.add_pinyin("su", &["苏", "素", "速", "宿", "诉"]);
        self.add_pinyin("suan", &["算", "酸"]);
        self.add_pinyin("sui", &["虽", "随", "岁", "碎"]);
        self.add_pinyin("sun", &["孙", "损", "笋"]);
        self.add_pinyin("suo", &["所", "锁", "索"]);

        self.add_pinyin("ta", &["他", "她", "它", "踏", "塔"]);
        self.add_pinyin("tai", &["太", "台", "态", "抬"]);
        self.add_pinyin("tan", &["谈", "坛", "弹", "探", "摊"]);
        self.add_pinyin("tang", &["堂", "糖", "汤", "躺", "趟"]);
        self.add_pinyin("tao", &["逃", "桃", "讨", "套", "陶"]);
        self.add_pinyin("te", &["特"]);
        self.add_pinyin("teng", &["疼", "腾"]);
        self.add_pinyin("ti", &["题", "提", "体", "替", "踢"]);
        self.add_pinyin("tian", &["天", "田", "填", "甜"]);
        self.add_pinyin("tiao", &["条", "跳", "调", "挑"]);
        self.add_pinyin("tie", &["铁", "贴"]);
        self.add_pinyin("ting", &["听", "停", "挺", "厅"]);
        self.add_pinyin("tong", &["同", "通", "痛", "统", "桶"]);
        self.add_pinyin("tou", &["头", "投", "透", "偷"]);
        self.add_pinyin("tu", &["土", "图", "突", "兔", "吐"]);
        self.add_pinyin("tuan", &["团", "团"]);
        self.add_pinyin("tui", &["退", "推", "腿"]);
        self.add_pinyin("tun", &["吞"]);
        self.add_pinyin("tuo", &["脱", "拖", "托"]);

        self.add_pinyin("wa", &["娃", "挖", "哇", "瓦"]);
        self.add_pinyin("wai", &["外", "歪"]);
        self.add_pinyin("wan", &["完", "晚", "玩", "万", "碗"]);
        self.add_pinyin("wang", &["王", "往", "网", "望", "忘"]);
        self.add_pinyin("wei", &["为", "位", "未", "围", "味"]);
        self.add_pinyin("wen", &["问", "文", "闻", "温", "稳"]);
        self.add_pinyin("weng", &["翁"]);
        self.add_pinyin("wo", &["我", "握", "窝"]);
        self.add_pinyin("wu", &["无", "五", "物", "屋", "误"]);

        self.add_pinyin("xi", &["西", "习", "喜", "系", "洗"]);
        self.add_pinyin("xia", &["下", "夏", "吓", "虾", "瞎"]);
        self.add_pinyin("xian", &["先", "现", "线", "县", "险"]);
        self.add_pinyin("xiang", &["想", "向", "像", "香", "响"]);
        self.add_pinyin("xiao", &["小", "笑", "校", "效", "消"]);
        self.add_pinyin("xie", &["些", "写", "谢", "血", "鞋"]);
        self.add_pinyin("xin", &["心", "新", "信", "欣"]);
        self.add_pinyin("xing", &["行", "星", "性", "姓", "兴"]);
        self.add_pinyin("xiong", &["兄", "熊", "胸"]);
        self.add_pinyin("xiu", &["修", "秀", "休", "袖"]);
        self.add_pinyin("xu", &["需", "许", "续", "须", "虚"]);
        self.add_pinyin("xuan", &["选", "宣", "悬", "旋"]);
        self.add_pinyin("xue", &["学", "雪", "血", "穴"]);
        self.add_pinyin("xun", &["讯", "寻", "训", "迅"]);

        self.add_pinyin("ya", &["呀", "牙", "压", "押", "鸭"]);
        self.add_pinyin("yan", &["眼", "言", "严", "研", "烟"]);
        self.add_pinyin("yang", &["样", "阳", "养", "洋", "杨"]);
        self.add_pinyin("yao", &["要", "药", "摇", "腰", "咬"]);
        self.add_pinyin("ye", &["也", "业", "夜", "叶", "野"]);
        self.add_pinyin("yi", &["一", "以", "已", "意", "衣"]);
        self.add_pinyin("yin", &["因", "音", "引", "印", "银"]);
        self.add_pinyin("ying", &["应", "影", "英", "营", "赢"]);
        self.add_pinyin("yo", &["哟"]);
        self.add_pinyin("yong", &["用", "永", "勇", "涌"]);
        self.add_pinyin("you", &["有", "又", "友", "由", "油"]);
        self.add_pinyin("yu", &["与", "于", "语", "鱼", "雨"]);
        self.add_pinyin("yuan", &["元", "原", "远", "院", "员"]);
        self.add_pinyin("yue", &["月", "越", "约", "乐"]);
        self.add_pinyin("yun", &["云", "运", "允", "晕"]);

        self.add_pinyin("za", &["杂", "砸"]);
        self.add_pinyin("zai", &["在", "再", "载", "灾"]);
        self.add_pinyin("zan", &["咱", "赞", "暂"]);
        self.add_pinyin("zang", &["脏", "藏"]);
        self.add_pinyin("zao", &["早", "造", "糟", "遭", "枣"]);
        self.add_pinyin("ze", &["则", "责", "择"]);
        self.add_pinyin("zei", &["贼"]);
        self.add_pinyin("zen", &["怎"]);
        self.add_pinyin("zeng", &["增", "曾"]);
        self.add_pinyin("zha", &["扎", "炸", "眨", "渣"]);
        self.add_pinyin("zhai", &["宅", "窄", "摘"]);
        self.add_pinyin("zhan", &["站", "战", "占", "展", "粘"]);
        self.add_pinyin("zhang", &["张", "长", "章", "账", "掌"]);
        self.add_pinyin("zhao", &["找", "照", "招", "着", "赵"]);
        self.add_pinyin("zhe", &["这", "着", "者", "折"]);
        self.add_pinyin("zhei", &["这"]);
        self.add_pinyin("zhen", &["真", "镇", "震", "针", "珍"]);
        self.add_pinyin("zheng", &["正", "整", "政", "证", "争"]);
        self.add_pinyin("zhi", &["只", "知", "之", "直", "至"]);
        self.add_pinyin("zhong", &["中", "种", "重", "众", "终"]);
        self.add_pinyin("zhou", &["周", "洲", "州", "粥", "舟"]);
        self.add_pinyin("zhu", &["主", "住", "注", "助", "祝"]);
        self.add_pinyin("zhua", &["抓"]);
        self.add_pinyin("zhuai", &["拽"]);
        self.add_pinyin("zhuan", &["转", "专", "赚", "砖"]);
        self.add_pinyin("zhuang", &["装", "状", "撞", "庄"]);
        self.add_pinyin("zhui", &["追", "坠"]);
        self.add_pinyin("zhun", &["准"]);
        self.add_pinyin("zhuo", &["桌", "捉", "着"]);
        self.add_pinyin("zi", &["自", "子", "字", "资", "紫"]);
        self.add_pinyin("zong", &["总", "宗", "综"]);
        self.add_pinyin("zou", &["走", "奏"]);
        self.add_pinyin("zu", &["组", "族", "足", "祖", "阻"]);
        self.add_pinyin("zuan", &["钻"]);
        self.add_pinyin("zui", &["最", "嘴", "罪", "醉"]);
        self.add_pinyin("zun", &["尊", "遵"]);
        self.add_pinyin("zuo", &["做", "作", "坐", "左", "座"]);
    }

    /// Add pinyin entry to dictionary
    fn add_pinyin(&mut self, pinyin: &str, chars: &[&str]) {
        let entries: Vec<String> = chars.iter().map(|s| s.to_string()).collect();
        self.dictionary.insert(pinyin.to_string(), entries);
    }

    /// Look up candidates for pinyin
    fn lookup(&self, pinyin: &str) -> Vec<Candidate> {
        let pinyin_lower = pinyin.to_ascii_lowercase();
        let mut candidates = Vec::new();

        // Exact match
        if let Some(chars) = self.dictionary.get(&pinyin_lower) {
            for (i, ch) in chars.iter().enumerate() {
                candidates.push(Candidate {
                    text: ch.clone(),
                    label: None,
                    annotation: Some(pinyin.to_string()),
                    score: (100 - i) as u32,
                });
            }
        }

        // Prefix match
        for (key, chars) in &self.dictionary {
            if key.starts_with(&pinyin_lower) && key != &pinyin_lower {
                for (i, ch) in chars.iter().take(3).enumerate() {
                    candidates.push(Candidate {
                        text: ch.clone(),
                        label: None,
                        annotation: Some(key.clone()),
                        score: (50 - i) as u32,
                    });
                }
            }
        }

        // Sort by score
        candidates.sort_by(|a, b| b.score.cmp(&a.score));
        candidates.truncate(self.config.max_candidates);
        candidates
    }

    /// Update candidates based on current buffer
    fn update_candidates(&mut self) {
        if self.pinyin_buffer.is_empty() {
            self.candidates.clear();
            self.state = InputMethodState::Idle;
        } else {
            self.candidates = self.lookup(&self.pinyin_buffer);
            self.selected = 0;
            self.state = if self.candidates.is_empty() {
                InputMethodState::Composing
            } else {
                InputMethodState::Selecting
            };
        }
    }
}

impl Default for PinyinEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InputMethodEngine for PinyinEngine {
    fn im_type(&self) -> InputMethodType {
        InputMethodType::Pinyin
    }

    fn process_key(&mut self, event: InputEvent) -> InputResult {
        if !event.is_press {
            return InputResult::NotHandled;
        }

        // Handle control keys
        if event.modifiers.ctrl || event.modifiers.alt {
            return InputResult::NotHandled;
        }

        let ch = match event.character {
            Some(c) => c,
            None => return InputResult::NotHandled,
        };

        match ch {
            // Letters add to pinyin buffer
            'a'..='z' | 'A'..='Z' => {
                self.pinyin_buffer.push(ch.to_ascii_lowercase());
                self.update_candidates();

                if self.candidates.is_empty() {
                    return InputResult::Preedit {
                        text: self.pinyin_buffer.clone(),
                        cursor: self.pinyin_buffer.len(),
                    };
                } else {
                    return InputResult::ShowCandidates(self.candidates.clone());
                }
            }

            // Number selects candidate
            '1'..='9' if !self.candidates.is_empty() => {
                let idx = (ch as usize) - ('1' as usize);
                if idx < self.candidates.len() {
                    let text = self.candidates[idx].text.clone();
                    self.reset();
                    return InputResult::Commit(text);
                }
            }

            // Space commits first candidate or preedit
            ' ' => {
                if !self.candidates.is_empty() {
                    let text = self.candidates[self.selected].text.clone();
                    self.reset();
                    return InputResult::Commit(text);
                } else if !self.pinyin_buffer.is_empty() {
                    let text = self.pinyin_buffer.clone();
                    self.reset();
                    return InputResult::Commit(text);
                }
            }

            // Backspace removes last character
            '\x08' | '\x7f' => {
                if !self.pinyin_buffer.is_empty() {
                    self.pinyin_buffer.pop();
                    self.update_candidates();

                    if self.pinyin_buffer.is_empty() {
                        return InputResult::HideCandidates;
                    } else if !self.candidates.is_empty() {
                        return InputResult::ShowCandidates(self.candidates.clone());
                    } else {
                        return InputResult::Preedit {
                            text: self.pinyin_buffer.clone(),
                            cursor: self.pinyin_buffer.len(),
                        };
                    }
                }
            }

            // Escape cancels
            '\x1b' => {
                if !self.pinyin_buffer.is_empty() {
                    self.reset();
                    return InputResult::HideCandidates;
                }
            }

            // Enter commits preedit as-is
            '\r' | '\n' => {
                if !self.pinyin_buffer.is_empty() {
                    let text = self.pinyin_buffer.clone();
                    self.reset();
                    return InputResult::Commit(text);
                }
            }

            _ => {}
        }

        InputResult::NotHandled
    }

    fn preedit(&self) -> &str {
        &self.pinyin_buffer
    }

    fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    fn state(&self) -> InputMethodState {
        self.state
    }

    fn reset(&mut self) {
        self.pinyin_buffer.clear();
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
        } else if !self.pinyin_buffer.is_empty() {
            let text = self.pinyin_buffer.clone();
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
