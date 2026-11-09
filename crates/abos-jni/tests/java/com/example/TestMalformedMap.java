package com.example;

import java.util.AbstractMap;
import java.util.Collections;
import java.util.Set;

/** A map whose raw entry set deliberately violates the Map.Entry contract. */
@SuppressWarnings({"rawtypes", "unchecked"})
public class TestMalformedMap extends AbstractMap {
    @Override
    public Set entrySet() {
        return Collections.singleton("not a map entry");
    }
}
