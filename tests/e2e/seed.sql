TRUNCATE write_keys CASCADE;
TRUNCATE projects CASCADE;
TRUNCATE identity_aliases CASCADE;

INSERT INTO projects (id, name) VALUES ('proj_test', 'Test Project');

INSERT INTO write_keys (project_id, key)
VALUES ('proj_test', 'wk_test_valid_key_12345');
