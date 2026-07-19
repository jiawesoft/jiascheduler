ALTER TABLE instance
ADD COLUMN `auth_type` varchar(40) NOT NULL DEFAULT "password" COMMENT 'auth_type: password, key_path, key_content',
ADD COLUMN `key_path` varchar(1000) NOT NULL DEFAULT '' COMMENT 'private key path',
ADD COLUMN `key_content` VARCHAR(5000) NOT NULL DEFAULT '' COMMENT 'private key content';