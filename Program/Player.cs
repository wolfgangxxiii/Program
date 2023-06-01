public abstract class Player
{
    public string Name { get; set; } // Nazwa gracza
    public char Symbol { get; set; } // Symbol gracza

    // Konstruktor klasy Player, inicjalizuje nazwê i symbol gracza
    public Player(string name, char symbol)
    {
        Name = name;
        Symbol = symbol;
    }

    // Abstrakcyjna metoda MakeMove, która zostanie zaimplementowana w klasach pochodnych
    public abstract void MakeMove(Board board);
}
