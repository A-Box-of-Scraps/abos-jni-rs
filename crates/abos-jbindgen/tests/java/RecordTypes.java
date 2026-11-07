package com.example;

@Marker(7)
public record RecordTypes(int value) {
    public static Marker marker() {
        return RecordTypes.class.getAnnotation(Marker.class);
    }

    public record Nested(RecordTypes[] values) {}

    public static class Member {
        public RecordTypes echo(RecordTypes value) {
            return value;
        }
    }
}
