use std::collections::HashMap;

/**
 * Трейт для работы с конфигами роутов для приложения.
 * Роут - это пара входящий адрес -- список адресатов, 
 * на который надо переслать запрос.
 * 
 * Пример:
 * Входящий запрос /foo/bar
 * Перенаправляем на:
 * https://some-host:9090/foo/bar,
 * https://another-host:9898/foo/bar 
 */
pub trait Config {
    /**
     * По урлу понимает на какие адреса надо переслать запросы
     */
    fn get_dests(&self, source_addr: &String) -> Option<&Vec<String>>;

    /**
     * Добавляет пару урл - новые адресаты 
     */
    fn add_dests(self, source_addr: String, destinations: Vec<String>);

    /**
     * Удаляет пару урл - новые адреса
     */
    fn delete_dests(self, source_addr: &String);
}

#[derive(Default)]
pub struct HashMapConfig {
    _dict: HashMap<String, Vec<String>>,
}

impl Config for HashMapConfig {
    fn get_dests(&self, source_addr: &String) -> Option<&Vec<String>> {
        self._dict.get(source_addr)
    }

    fn add_dests(mut self, source_addr: String, destinations: Vec<String>) {
        self._dict.insert(source_addr, destinations);
    }

    fn delete_dests(mut self, source_addr: &String) {
        self._dict.remove(source_addr);
    }
}