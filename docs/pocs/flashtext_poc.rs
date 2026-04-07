use std::collections::HashMap;

/// Trie 節點結構
#[derive(Default, Debug)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    // 如果這個節點是一個關鍵字的結尾，儲存該關鍵字 (或是標準化後的替換詞)
    keyword: Option<String>,
}

/// FlashText 演算法的 Rust 極簡實作
#[derive(Default)]
pub struct FlashText {
    root: TrieNode,
}

impl FlashText {
    pub fn new() -> Self {
        Self::default()
    }

    /// 將關鍵字加入 Trie 字典樹
    pub fn add_keyword(&mut self, keyword: &str) {
        let mut current = &mut self.root;
        for ch in keyword.chars() {
            // 逐字元建立子節點
            current = current.children.entry(ch).or_default();
        }
        // 在單詞結尾標記
        current.keyword = Some(keyword.to_string());
    }

    /// 在文本中快速提取所有匹配的關鍵字 (O(N) 時間複雜度)
    pub fn extract_keywords(&self, text: &str) -> Vec<String> {
        let mut results = Vec::new();
        // 將字串轉為字元陣列以支援完美的 UTF-8 (如繁體中文) 解析
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let mut current = &self.root;
            let mut j = i;
            let mut longest_match: Option<String> = None;
            let mut match_end = i;

            // 從位置 i 開始往下找
            while j < chars.len() {
                if let Some(node) = current.children.get(&chars[j]) {
                    current = node;
                    if let Some(ref kw) = current.keyword {
                        // 找到一個匹配！記錄下來，繼續往下看有沒有更長的匹配
                        // (例如字典有 "Apple" 和 "Apple Watch"，要優先匹配長詞)
                        longest_match = Some(kw.clone());
                        match_end = j;
                    }
                    j += 1;
                } else {
                    break; // 字典樹斷了，停止匹配
                }
            }

            if let Some(kw) = longest_match {
                // 如果有找到匹配，把結果推入陣列
                results.push(kw);
                // 直接跳過已經匹配的字元長度 (這是 FlashText 高效的關鍵之一)
                i = match_end + 1; 
            } else {
                // 沒找到，游標往前移一格
                i += 1;
            }
        }
        
        results
    }
}

fn main() {
    let mut ft = FlashText::new();
    
    // 建立我們的領域字典 (Domain Dictionary)
    ft.add_keyword("Rust");
    ft.add_keyword("反脆弱");
    ft.add_keyword("Agent");
    ft.add_keyword("Cueward");
    ft.add_keyword("Apple Notes");

    // 模擬一段從 Safari 或備忘錄抓取出來的超長字串
    let text = "今天我用 Rust 寫了 Cueward 這個 Agent 工具，並且把昨天在 Apple Notes 裡記錄的反脆弱系統架構設計給實作出來了！這速度真的太快了。";
    
    println!("📜 原始文本: {}", text);
    
    let start = std::time::Instant::now();
    let found = ft.extract_keywords(text);
    let duration = start.elapsed();
    
    println!("✨ 提取到的關鍵字 (Cue Tags): {:?}", found);
    println!("⏱️ 執行時間: {:?}", duration);
}
