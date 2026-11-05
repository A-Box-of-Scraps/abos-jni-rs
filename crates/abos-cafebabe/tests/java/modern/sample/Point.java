package sample;

record Point(int x, int y) implements Shape {
    public String label() {
        return "point: " + x;
    }
}
