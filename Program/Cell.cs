public class Cell : GameEntity
{
    public char Value { get; set; } // W�a�ciwo�� przechowuj�ca warto�� kom�rki

    public Cell()
    {
        Value = ' '; // Inicjalizacja warto�ci kom�rki jako spacja
    }
}
