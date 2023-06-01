public class Board : GameEntity
{
    private readonly Cell[,] _cells = new Cell[3, 3]; // Dwuwymiarowa tablica kom�rek reprezentuj�cych plansz�

    public Board()
    {
        for (int i = 0; i < 3; i++) // Inicjalizacja tablicy kom�rek planszy
        {
            for (int j = 0; j < 3; j++)
            {
                _cells[i, j] = new Cell();
            }
        }
    }

    public void Display()
    {
        Console.Write(" "); // Wci�cie dla numer�w wierszy
        for (int k = 0; k < 3; k++)
        {
            Console.Write($" {k + 1} "); // Wy�wietlanie numer�w kolumn
        }
        Console.WriteLine();

        for (int i = 0; i < 3; i++)
        {
            Console.Write($"{i + 1} "); // Wy�wietlanie numer�w wierszy
            for (int j = 0; j < 3; j++)
            {
                Console.Write($"|{_cells[i, j].Value}"); // Wy�wietlanie warto�ci kom�rki planszy
            }
            Console.WriteLine("|");
            if (i < 2)
            {
                Console.WriteLine("  -------"); // Dodanie wci�cia dla numer�w wierszy
            }
        }
    }

    public bool PlaceSymbol(int x, int y, char symbol)
    {
        if (_cells[x, y].Value == ' ') // Sprawdzenie, czy kom�rka jest pusta
        {
            _cells[x, y].Value = symbol; // Ustawienie symbolu w kom�rce
            return true; // Zwr�cenie informacji o poprawnym umieszczeniu symbolu
        }
        return false; // Zwr�cenie informacji o niepoprawnym umieszczeniu symbolu
    }

    public bool CheckForWinner(char symbol)
    {
        for (int i = 0; i < 3; i++)
        {
            if ((_cells[i, 0].Value == symbol && _cells[i, 1].Value == symbol && _cells[i, 2].Value == symbol) || // Sprawdzenie wygranej w wierszach
                (_cells[0, i].Value == symbol && _cells[1, i].Value == symbol && _cells[2, i].Value == symbol)) // Sprawdzenie wygranej w kolumnach
            {
                return true; // Zwr�cenie informacji o wygranej
            }
        }

        if ((_cells[0, 0].Value == symbol && _cells[1, 1].Value == symbol && _cells[2, 2].Value == symbol) || // Sprawdzenie wygranej na przek�tnej
            (_cells[0, 2].Value == symbol && _cells[1, 1].Value == symbol && _cells[2, 0].Value == symbol)) // Sprawdzenie wygranej na przek�tnej
        {
            return true; // Zwr�cenie informacji o wygranej
        }

        return false; // Zwr�cenie informacji o braku wygranej
    }
}
