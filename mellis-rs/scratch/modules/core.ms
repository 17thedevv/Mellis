
module std {
    export enum Option<T> {
        Some(T),
        None,
    }
    
    export module collections {
        export struct Map<K, V> {
            k: K,
            v: V,
        }
    }
}
