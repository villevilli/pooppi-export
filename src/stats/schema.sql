DROP TABLE IF EXISTS stats;
DROP TABLE IF EXISTS objectives;
DROP TABLE IF EXISTS players;
CREATE TABLE objectives (
    id BIGINT NOT NULL AUTO_INCREMENT,
    objective_name VARCHAR(255) UNIQUE,
    display_name VARCHAR(255),
    criteria_name VARCHAR(255),
    PRIMARY KEY (id)
);
CREATE TABLE players (
    id BIGINT NOT NULL AUTO_INCREMENT,
    player_name VARCHAR(255) UNIQUE,
    PRIMARY KEY (id)
);
CREATE TABLE stats (
    score BIGINT,
    player_name VARCHAR(255),
    objective_name VARCHAR(255),
    time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (player_name) REFERENCES players (player_name),
    FOREIGN KEY (objective_name) REFERENCES objectives (objective_name)
) ENGINE InnoDB PAGE_COMPRESSED = 1 PAGE_COMPRESSION_LEVEL = 9;