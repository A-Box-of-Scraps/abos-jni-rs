package com.example;

public class MemberCollisions {
    public int newInt;
    public int make;
    public int __;

    public MemberCollisions(int value) {
        newInt = value;
    }

    public MemberCollisions(String value) {
        newInt = value.length();
    }

    public int newInt() {
        return newInt + 100;
    }

    public int setNewInt_(int value) {
        return value + 200;
    }

    public int set___() {
        return __ + 300;
    }

    public int choose(int value) {
        return value + 10;
    }

    public int choose(String value) {
        return value.length() + 20;
    }
}
