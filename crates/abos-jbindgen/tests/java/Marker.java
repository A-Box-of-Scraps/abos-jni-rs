package com.example;

import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;

@Retention(RetentionPolicy.RUNTIME)
public @interface Marker {
    int value();

    @interface Nested {
        Marker[] value();
    }

    record Member(Marker[] values) {}
}
