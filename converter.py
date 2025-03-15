import json
import requests
import os
import sqlite3
from sqlite3 import Error
from typing import Optional, Dict, List

# 类型定义
class SentenceData:
    def __init__(self, data: Dict):
        self.uuid = data["uuid"]
        self.text = data["hitokoto"]
        self.type = data["type"]
        self.from_source = data["from"]
        self.from_who = data.get("from_who", "")
        self.length = data["length"]

def create_connection() -> Optional[sqlite3.Connection]:
    """创建数据库连接"""
    try:
        return sqlite3.connect("hitokoto.db", isolation_level=None)
    except Error as e:
        print(f"数据库连接失败: {e}")
        return None

def create_table(conn: sqlite3.Connection) -> None:
    """创建数据表（带自增ID）"""
    try:
        cursor = conn.cursor()
        # 先删除旧表（如果存在）
        cursor.execute("DROP TABLE IF EXISTS hitokoto")
        # 创建新表结构
        cursor.execute('''
            CREATE TABLE hitokoto (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uuid TEXT UNIQUE NOT NULL,
                text TEXT NOT NULL,
                type TEXT NOT NULL,
                from_source TEXT NOT NULL,
                from_who TEXT DEFAULT '',
                length INTEGER NOT NULL
            );
        ''')
        # 创建索引提升查询性能
        cursor.execute("CREATE INDEX index_hitokoto ON hitokoto (type, length)")
        print("数据表创建成功")
    except Error as e:
        print(f"创建表失败: {e}")
        conn.rollback()

def batch_insert_sentences(conn: sqlite3.Connection, sentences: List[SentenceData]) -> None:
    """批量插入数据"""
    try:
        cursor = conn.cursor()
        # 开启事务
        cursor.execute("BEGIN TRANSACTION")
        
        # 准备批量插入数据
        data = [
            (
                s.uuid,
                s.text,
                s.type,
                s.from_source,
                s.from_who,
                s.length,
            )
            for s in sentences
        ]
        
        # 使用批量插入
        cursor.executemany('''
            INSERT OR IGNORE INTO hitokoto 
            (uuid, text, type, from_source, from_who, length)
            VALUES (?, ?, ?, ?, ?, ?)
        ''', data)
        
        # 提交事务
        conn.commit()
        print(f"成功插入 {cursor.rowcount} 条记录")
        
    except Error as e:
        print(f"数据插入失败: {e}")
        conn.rollback()

def get_version() -> Optional[Dict]:
    """获取版本数据"""
    try:
        response = requests.get(
            "https://sentences-bundle.hitokoto.cn/version.json",
            timeout=10
        )
        response.raise_for_status()
        return response.json()
    except requests.RequestException as e:
        print(f"获取版本信息失败: {e}")
        return None

def fetch_category_data(key: str, name: str) -> Optional[List[Dict]]:
    """获取分类数据"""
    try:
        url = f"https://sentences-bundle.hitokoto.cn/sentences/{key}.json"
        response = requests.get(url, timeout=15)
        response.raise_for_status()
        print(f"成功下载 {name} 数据")
        return response.json()
    except requests.RequestException as e:
        print(f"下载 {name} 数据失败: {e}")
        return None

def main():
    os.remove("./hitokoto.db")
    # 初始化数据库
    conn = create_connection()
    if not conn:
        return

    # 创建数据表
    create_table(conn)
    
    # 获取版本信息
    version_data = get_version()
    if not version_data:
        conn.close()
        return
    
    # 处理每个分类
    total_inserted = 0
    for category in version_data.get("sentences", []):
        key = category.get("key")
        name = category.get("name")
        if not key or not name:
            continue
            
        print(f"\n正在处理分类: {name}")
        
        # 获取句子数据
        sentences = fetch_category_data(key, name)
        if not sentences:
            continue
            
        # 准备数据对象
        sentence_objects = [SentenceData(s) for s in sentences]
        
        # 批量插入
        batch_insert_sentences(conn, sentence_objects)
        total_inserted += len(sentence_objects)
    
    # 收尾工作
    print(f"\n操作完成，总计处理 {total_inserted} 条记录")
    conn.close()

if __name__ == "__main__":
    main()