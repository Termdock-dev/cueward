import Foundation
import SQLite3

func getRecentMessages() {
    let home = FileManager.default.homeDirectoryForCurrentUser
    let dbPath = home.appendingPathComponent("Library/Messages/chat.db").path
    
    var db: OpaquePointer?
    if sqlite3_open(dbPath, &db) != SQLITE_OK {
        print("Error: Could not open Messages database at \(dbPath)")
        print("Note: You may need 'Full Disk Access' for your Terminal app.")
        return
    }
    
    // 查詢最近 5 則訊息及其發送者
    let query = """
    SELECT 
        message.text, 
        handle.id as sender, 
        datetime(message.date / 1000000000 + strftime('%s', '2001-01-01'), 'unixepoch', 'localtime') as date
    FROM message 
    JOIN handle ON message.handle_id = handle.ROWID 
    WHERE message.text IS NOT NULL
    ORDER BY message.date DESC 
    LIMIT 5;
    """
    
    var statement: OpaquePointer?
    if sqlite3_prepare_v2(db, query, -1, &statement, nil) == SQLITE_OK {
        var results: [[String: String]] = []
        
        while sqlite3_step(statement) == SQLITE_ROW {
            let text = String(cString: sqlite3_column_text(statement, 0))
            let sender = String(cString: sqlite3_column_text(statement, 1))
            let date = String(cString: sqlite3_column_text(statement, 2))
            
            results.append([
                "date": date,
                "sender": sender,
                "text": text
            ])
        }
        
        if let jsonData = try? JSONSerialization.data(withJSONObject: results, options: .prettyPrinted),
           let jsonString = String(data: jsonData, encoding: .utf8) {
            print(jsonString)
        }
    } else {
        print("Error: Failed to prepare SQL query.")
    }
    
    sqlite3_finalize(statement)
    sqlite3_close(db)
}

getRecentMessages()
