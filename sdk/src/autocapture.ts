export class Autocapture {
  attach(onEvent: (name: string) => void): void {
    document.addEventListener('click', () => {
      onEvent('Element Clicked')
    })
  }

  detach(): void {
    // stub
  }
}
