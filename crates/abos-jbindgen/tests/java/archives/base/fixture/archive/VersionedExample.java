package fixture.archive;

public class VersionedExample {
    public static class Inner {}

    public static int version() {
        return 9;
    }

    int baseOnly() {
        return 9;
    }
}
