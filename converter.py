import json
import os
import requests
from typing import Optional, Dict, List
import pymysql
from pymysql import Error


# 类型定义
class SentenceData:
    def __init__(self, data: Dict):
        self.uuid = data["uuid"]
        self.text = data["hitokoto"]
        self.type = data["type"]
        self.from_source = data["from"]
        self.from_who = data.get("from_who", "")
        self.length = data["length"]


def create_connection() -> Optional[pymysql.connections.Connection]:
    """创建数据库连接"""
    try:
        conn = pymysql.connect(
            host="localhost",
            user="root",
            password="yo12345678",
        )
        cursor = conn.cursor()
        cursor.execute("CREATE DATABASE IF NOT EXISTS hitokoto")
        conn.select_db("hitokoto")
        return conn
    except Error as e:
        print(f"数据库连接失败: {e}")
        return None


def create_table(conn: pymysql.connections.Connection) -> None:
    """创建数据表（带自增ID）"""
    try:
        cursor = conn.cursor()
        cursor.execute("DROP TABLE IF EXISTS hitokoto")
        cursor.execute(
            """
            CREATE TABLE hitokoto (
                id INT PRIMARY KEY AUTO_INCREMENT,
                uuid VARCHAR(36) UNIQUE NOT NULL,
                text TEXT NOT NULL,
                type VARCHAR(50) NOT NULL,
                from_source TEXT NOT NULL,
                from_who TEXT,
                length INT NOT NULL
            );
        """
        )
        cursor.execute("CREATE INDEX index_hitokoto ON hitokoto (type, length)")
        print("数据表创建成功")
    except Error as e:
        print(f"创建表失败: {e}")
        conn.rollback()


def batch_insert_sentences(
    conn: pymysql.connections.Connection, sentences: List[SentenceData]
) -> None:
    """批量插入数据"""
    try:
        cursor = conn.cursor()
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
        cursor.executemany(
            """
            INSERT IGNORE INTO hitokoto 
            (uuid, text, type, from_source, from_who, length)
            VALUES (%s, %s, %s, %s, %s, %s)
        """,
            data,
        )
        conn.commit()
        print(f"成功插入 {cursor.rowcount} 条记录")
    except Error as e:
        print(f"数据插入失败: {e}")
        conn.rollback()


def get_version() -> Optional[Dict]:
    """获取版本数据"""

    try:
        # 从网络获取
        response = requests.get(
            "https://github.com/hitokoto-osc/sentences-bundle/raw/refs/heads/master/version.json",
            timeout=10,
        )
        response.raise_for_status()
        version_data = response.json()

        return version_data
    except requests.RequestException as e:
        print(f"获取版本信息失败: {e}")
        return None


def fetch_category_data(
    key: str, name: str, category_timestamp: int
) -> Optional[List[Dict]]:
    """获取分类数据"""
    cache_dir = "./cache"
    os.makedirs(cache_dir, exist_ok=True)
    category_cache_path = os.path.join(cache_dir, f"{key}.json")

    try:
        # 先尝试从缓存加载分类数据
        if os.path.exists(category_cache_path):
            with open(category_cache_path, "r", encoding="utf-8") as f:
                cached_data = json.load(f)

            # 检查缓存是否需要更新
            cached_timestamp = cached_data.get("timestamp", 0)
            if cached_timestamp >= category_timestamp:
                print(f"缓存的 {name} 数据是最新的，无需更新")
                return cached_data["sentences"]

        # 如果缓存不存在或需要更新，则从网络获取
        url = f"https://github.com/hitokoto-osc/sentences-bundle/raw/refs/heads/master/sentences/{key}.json"
        response = requests.get(url, timeout=15, verify=False)
        response.raise_for_status()
        print(f"成功下载 {name} 数据")
        data = response.json()

        # 保存到缓存
        with open(category_cache_path, "w", encoding="utf-8") as f:
            json.dump(
                {"timestamp": category_timestamp, "sentences": data},
                f,
                ensure_ascii=False,
                indent=4,
            )

        return data
    except requests.RequestException as e:
        print(f"下载 {name} 数据失败: {e}")
        return None


def main():
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

        # 获取分类的时间戳
        category_timestamp = category.get("timestamp", 0)

        # 获取句子数据
        sentences = fetch_category_data(key, name, category_timestamp)
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
