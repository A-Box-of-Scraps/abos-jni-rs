package com.example;

import example.annotations.NonNull;

public class PlatformProbe {
    @NonNull
    public String value(String documentedKey) {
        throw new RuntimeException("Stub!");
    }
}
